use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::RegexSettings;
use crate::consts::{MAX_REGEX_PATTERN_BYTES, REGEX_MATCH_MAX_IN_FLIGHT, REGEX_MATCH_TIMEOUT_MS};

use regex::{Regex, RegexBuilder};
use tracing::warn;

// ======================================================================
// コンパイルキャッシュ
// ======================================================================
/// 正規表現パターンのコンパイル結果を保持するキャッシュ
///
/// パターンとフラグが変わらない限り、コンパイル済み `Regex` を再利用する
#[derive(Debug, Default)]
pub(crate) struct RegexPatternCache {
    key: Option<RegexCompileKey>,
    compiled: Option<Regex>,
}

/// コンパイル結果のキャッシュキー (`replacement` はコンパイルに影響しない)
#[derive(Clone, Debug, PartialEq, Eq)]
struct RegexCompileKey {
    pattern: String,
    case_insensitive: bool,
    multiline: bool,
}

impl RegexPatternCache {
    /// 設定に対応するコンパイル済み正規表現を返す
    ///
    /// 設定が前回と同じ場合はキャッシュを再利用する
    pub(crate) fn get_or_compile(&mut self, settings: &RegexSettings) -> Option<&Regex> {
        let key = RegexCompileKey {
            pattern: settings.pattern.clone(),
            case_insensitive: settings.case_insensitive,
            multiline: settings.multiline,
        };

        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.compiled = compile_pattern(settings);
        }

        self.compiled.as_ref()
    }
}

// ======================================================================
// 正規表現コンパイル
// ======================================================================
/// 設定から正規表現をコンパイルする
///
/// パターンが空、上限超過、または構文エラーの場合は `None` を返す
fn compile_pattern(settings: &RegexSettings) -> Option<Regex> {
    if settings.pattern.is_empty() || settings.pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return None;
    }

    RegexBuilder::new(&settings.pattern)
        .case_insensitive(settings.case_insensitive)
        .multi_line(settings.multiline)
        .build()
        .ok()
}

// ======================================================================
// マッチ実行タイムアウト
// ======================================================================
/// 正規表現マッチ操作のタイムアウト
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegexMatchTimeout;

/// タイムアウト時に実行側へ中断を伝えるフラグ
///
/// `regex` クレートは単一の `find` 実行中を打ち切れないため、マッチ境界での
/// 協調キャンセルに使う
struct MatchCancel {
    cancelled: AtomicBool,
}

impl MatchCancel {
    /// 未キャンセル状態で生成する
    fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    /// キャンセルが要求されているか
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// キャンセルを要求する
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// 正規表現マッチの同時実行スロットを管理するプール
///
/// タイムアウト時はキャンセルを要求し、実行側がマッチ境界で中断してスロットを返す。
/// スロット数を超えて spawn しない
struct RegexMatchPool {
    available: Mutex<usize>,
    cv: Condvar,
}

/// 取得済みスロット。`Drop` 時にプールへ返却する
struct RegexMatchPermit {
    pool: &'static RegexMatchPool,
}

impl Drop for RegexMatchPermit {
    fn drop(&mut self) {
        self.pool.release();
    }
}

impl RegexMatchPool {
    /// 空きスロット数を上限で初期化する
    fn new(max_in_flight: usize) -> Self {
        Self {
            available: Mutex::new(max_in_flight),
            cv: Condvar::new(),
        }
    }

    /// 制限時間内にスロットを取得する
    ///
    /// # Arguments
    /// * `timeout` - 取得待ちの上限時間
    ///
    /// # Returns
    /// * `Result<RegexMatchPermit, RegexMatchTimeout>` - 取得成功時は `Permit`。
    ///   制限時間内に空きがなければタイムアウト
    fn acquire(&'static self, timeout: Duration) -> Result<RegexMatchPermit, RegexMatchTimeout> {
        let deadline = Instant::now() + timeout;
        let mut available = self.available.lock().map_err(|_| RegexMatchTimeout)?;

        loop {
            if *available > 0 {
                *available -= 1;
                return Ok(RegexMatchPermit { pool: self });
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(RegexMatchTimeout);
            }

            let (guard, wait_result) = self
                .cv
                .wait_timeout(available, remaining)
                .map_err(|_| RegexMatchTimeout)?;
            available = guard;

            if wait_result.timed_out() && *available == 0 {
                return Err(RegexMatchTimeout);
            }
        }
    }

    /// スロットを返却し待機中の取得を起こす
    fn release(&self) {
        if let Ok(mut available) = self.available.lock() {
            *available += 1;
            self.cv.notify_one();
        }
    }

    /// 現在実行中のスロット数を返す (ログ用)
    fn in_flight(&self) -> usize {
        self.available
            .lock()
            .map_or(REGEX_MATCH_MAX_IN_FLIGHT, |available| {
                REGEX_MATCH_MAX_IN_FLIGHT.saturating_sub(*available)
            })
    }
}

/// プロセス全体で共有する正規表現マッチ用プール
static REGEX_MATCH_POOL: LazyLock<RegexMatchPool> =
    LazyLock::new(|| RegexMatchPool::new(REGEX_MATCH_MAX_IN_FLIGHT));

/// 正規表現マッチを制限時間内で実行する
///
/// 制限時間を超えた場合はキャンセルを要求して `Err(RegexMatchTimeout)` を返す。
/// 実行側は `MatchCancel` を見てマッチ境界で中断する。同時実行は
/// `REGEX_MATCH_MAX_IN_FLIGHT` で上限する
fn run_with_match_timeout<T, F>(op: F) -> Result<T, RegexMatchTimeout>
where
    T: Send + 'static,
    F: FnOnce(&MatchCancel) -> Result<T, RegexMatchTimeout> + Send + 'static,
{
    run_with_match_timeout_duration(Duration::from_millis(REGEX_MATCH_TIMEOUT_MS), op)
}

/// 指定した制限時間で正規表現マッチを実行する
///
/// スロット取得に使った時間を差し引いて結果待ちする。取得できなければ spawn しない。
/// 待ちが切れたらキャンセルを要求し、実行側の早期終了を促す
fn run_with_match_timeout_duration<T, F>(timeout: Duration, op: F) -> Result<T, RegexMatchTimeout>
where
    T: Send + 'static,
    F: FnOnce(&MatchCancel) -> Result<T, RegexMatchTimeout> + Send + 'static,
{
    let started = Instant::now();
    let permit = REGEX_MATCH_POOL.acquire(timeout)?;
    let remaining = timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        // Permit をここで Drop し、未使用スロットを即座に返却する
        drop(permit);
        return Err(RegexMatchTimeout);
    }

    let cancel = Arc::new(MatchCancel::new());
    let cancel_worker = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("clip-refiner-regex".into())
        .spawn(move || {
            let _permit = permit;
            let _ = tx.send(op(&cancel_worker));
        })
        .map_err(|_| RegexMatchTimeout)?;

    match rx.recv_timeout(remaining) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(RegexMatchTimeout)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(RegexMatchTimeout)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel.cancel();
            Err(RegexMatchTimeout)
        }
    }
}

/// キャンセル要求を確認し、要求済みならタイムアウトを返す
fn check_cancel(cancel: &MatchCancel) -> Result<(), RegexMatchTimeout> {
    if cancel.is_cancelled() {
        Err(RegexMatchTimeout)
    } else {
        Ok(())
    }
}

/// キャンセル可能な置換を実行する
fn replace_all_cancellable(
    re: &Regex,
    text: &str,
    replacement: &str,
    cancel: &MatchCancel,
) -> Result<String, RegexMatchTimeout> {
    check_cancel(cancel)?;
    let mut out = String::new();
    let mut last = 0;
    for caps in re.captures_iter(text) {
        check_cancel(cancel)?;
        let m = caps.get(0).expect("captures_iter は全体マッチを持つ");
        out.push_str(&text[last..m.start()]);
        caps.expand(replacement, &mut out);
        last = m.end();
    }
    check_cancel(cancel)?;
    out.push_str(&text[last..]);
    Ok(out)
}

/// キャンセル可能な抽出を実行する
fn extract_cancellable(
    re: &Regex,
    text: &str,
    cancel: &MatchCancel,
) -> Result<Option<String>, RegexMatchTimeout> {
    check_cancel(cancel)?;
    let mut matches = Vec::new();
    for m in re.find_iter(text) {
        check_cancel(cancel)?;
        matches.push(m.as_str());
    }
    check_cancel(cancel)?;
    if matches.is_empty() {
        Ok(None)
    } else {
        Ok(Some(matches.join("\n")))
    }
}

/// キャンセル可能な分割を実行する
fn split_cancellable(
    re: &Regex,
    text: &str,
    cancel: &MatchCancel,
) -> Result<String, RegexMatchTimeout> {
    check_cancel(cancel)?;
    let mut parts = Vec::new();
    let mut last = 0;
    for m in re.find_iter(text) {
        check_cancel(cancel)?;
        parts.push(&text[last..m.start()]);
        last = m.end();
    }
    check_cancel(cancel)?;
    parts.push(&text[last..]);
    Ok(parts.join("\n"))
}

/// タイムアウト時に警告を出し、元テキストを返す
fn on_match_timeout(text: &str) -> Cow<'_, str> {
    warn!(
        timeout_ms = REGEX_MATCH_TIMEOUT_MS,
        in_flight = REGEX_MATCH_POOL.in_flight(),
        max_in_flight = REGEX_MATCH_MAX_IN_FLIGHT,
        "正規表現の実行がタイムアウトしたため元のテキストを維持する"
    );
    Cow::Borrowed(text)
}

// ======================================================================
// プレビュー
// ======================================================================
/// 設定 UI 向けプレビュー用サンプル文字列
pub const REGEX_PREVIEW_SAMPLE: &str = "Hello world 123";

/// 正規表現プレビューの結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexPreviewResult {
    /// パターンがコンパイル可能かどうか
    pub valid: bool,
    /// 状態メッセージ
    pub message: String,
    /// 置換後テキスト (`valid` かつパターン非空のとき)
    pub result: Option<String>,
}

/// サンプル文字列へ正規表現置換を適用してプレビュー結果を返す
///
/// 実行時の `regex_replace` と同じ `regex` クレートで評価する
/// マッチ実行が制限時間を超えた場合は無効として扱う
///
/// # Arguments
/// * `sample` - プレビュー対象のサンプル文字列
/// * `settings` - 正規表現パターンと置換文字列
///
/// # Returns
/// * `RegexPreviewResult` - パターンの有効性、状態メッセージ、置換後テキスト
pub fn preview_regex_replace(sample: &str, settings: &RegexSettings) -> RegexPreviewResult {
    if settings.pattern.is_empty() {
        return RegexPreviewResult {
            valid: true,
            message: "パターンが空です (マッチしません)".to_string(),
            result: Some(sample.to_string()),
        };
    }

    if settings.pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return RegexPreviewResult {
            valid: false,
            message: format!("パターンが長すぎます (最大 {MAX_REGEX_PATTERN_BYTES} バイト)"),
            result: None,
        };
    }

    match RegexBuilder::new(&settings.pattern)
        .case_insensitive(settings.case_insensitive)
        .multi_line(settings.multiline)
        .build()
    {
        Ok(re) => {
            let sample_owned = sample.to_owned();
            let replacement = settings.replacement.clone();
            match run_with_match_timeout(move |cancel| {
                replace_all_cancellable(&re, &sample_owned, &replacement, cancel)
            }) {
                Ok(result) => RegexPreviewResult {
                    valid: true,
                    message: "パターンは有効です".to_string(),
                    result: Some(result),
                },
                Err(RegexMatchTimeout) => RegexPreviewResult {
                    valid: false,
                    message: format!(
                        "パターンの実行がタイムアウトしました (上限 {REGEX_MATCH_TIMEOUT_MS} ms)"
                    ),
                    result: None,
                },
            }
        }
        Err(err) => RegexPreviewResult {
            valid: false,
            message: format!("パターンが無効です: {err}"),
            result: None,
        },
    }
}

// ======================================================================
// 置換
// ======================================================================
/// 正規表現に一致する部分を置換文字列へ変換する
///
/// パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターンと置換文字列
///
/// # Returns
/// * `Cow<'_, str>` - 置換後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_replace<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    let replacement = settings.replacement.clone();
    match run_with_match_timeout(move |cancel| {
        replace_all_cancellable(&re, &owned_text, &replacement, cancel)
    }) {
        Ok(result) if result == text => Cow::Borrowed(text),
        Ok(result) => Cow::Owned(result),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// 抽出
// ======================================================================
/// 正規表現に一致する部分を行単位で抽出する
///
/// 一致がない場合、パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は
/// 元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターン
///
/// # Returns
/// * `Cow<'_, str>` - 抽出結果。変更がない場合は元のテキストを借用
pub(crate) fn regex_extract<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| extract_cancellable(&re, &owned_text, cancel)) {
        Ok(None) => Cow::Borrowed(text),
        Ok(Some(joined)) if joined == text => Cow::Borrowed(text),
        Ok(Some(joined)) => Cow::Owned(joined),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// 削除
// ======================================================================
/// 正規表現に一致する部分を削除する
///
/// パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターン
///
/// # Returns
/// * `Cow<'_, str>` - 削除後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_delete<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| {
        replace_all_cancellable(&re, &owned_text, "", cancel)
    }) {
        Ok(result) if result == text => Cow::Borrowed(text),
        Ok(result) => Cow::Owned(result),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// 分割
// ======================================================================
/// 正規表現でテキストを分割し、改行で結合する
///
/// パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターン
///
/// # Returns
/// * `Cow<'_, str>` - 分割後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_split<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| split_cancellable(&re, &owned_text, cancel)) {
        Ok(joined) if joined == text => Cow::Borrowed(text),
        Ok(joined) => Cow::Owned(joined),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Instant;

    use super::*;

    use crate::consts::REGEX_MATCH_MAX_IN_FLIGHT;

    /// グローバルプールを触るテストの直列化用
    static POOL_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// プールが空くまで待ってからクロージャを実行し、終了後も空くまで待つ
    fn with_pool_test_lock<T>(f: impl FnOnce() -> T) -> T {
        let _guard = POOL_TEST_LOCK.lock().expect("regex pool test lock");
        wait_until_pool_idle();
        let result = f();
        wait_until_pool_idle();
        result
    }

    /// 実行中スロットが 0 になるまで短周期で待つ
    fn wait_until_pool_idle() {
        let deadline = Instant::now() + Duration::from_secs(2);
        while REGEX_MATCH_POOL.in_flight() > 0 {
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            REGEX_MATCH_POOL.in_flight(),
            0,
            "正規表現マッチプールがアイドルに戻ること"
        );
    }

    fn settings(pattern: &str) -> RegexSettings {
        RegexSettings {
            pattern: pattern.to_string(),
            ..RegexSettings::default()
        }
    }

    fn settings_with_replacement(pattern: &str, replacement: &str) -> RegexSettings {
        RegexSettings {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            ..RegexSettings::default()
        }
    }

    fn mut_cache() -> RegexPatternCache {
        RegexPatternCache::default()
    }

    /// 置換の基本動作できるようにすること
    #[test]
    fn test_regex_replace() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(
                regex_replace("a1b2c", &settings_with_replacement(r"\d", "X"), &mut cache),
                "aXbXc"
            );
            assert_eq!(
                regex_replace("plain", &settings(r"\d"), &mut cache),
                "plain"
            );
        });
    }

    /// キャプチャグループを使った置換できるようにすること
    #[test]
    fn test_regex_replace_capture() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(
                regex_replace(
                    "2024-01-02",
                    &settings_with_replacement(r"(\d+)-(\d+)", r"$2/$1"),
                    &mut cache
                ),
                "01/2024-02"
            );
        });
    }

    /// 抽出の基本動作できるようにすること
    #[test]
    fn test_regex_extract() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(
                regex_extract("a1b22c", &settings(r"\d+"), &mut cache),
                "1\n22"
            );
            assert_eq!(regex_extract("abc", &settings(r"\d+"), &mut cache), "abc");
        });
    }

    /// 削除の基本動作できるようにすること
    #[test]
    fn test_regex_delete() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(regex_delete("a1b2c", &settings(r"\d"), &mut cache), "abc");
        });
    }

    /// 分割の基本動作できるようにすること
    #[test]
    fn test_regex_split() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(regex_split("a,b,c", &settings(r","), &mut cache), "a\nb\nc");
            assert_eq!(
                regex_split("a  b   c", &settings(r"\s+"), &mut cache),
                "a\nb\nc"
            );
        });
    }

    /// 無効なパターンは元のテキストを返すようにすること
    #[test]
    fn test_invalid_pattern_returns_original() {
        with_pool_test_lock(|| {
            let invalid = RegexSettings {
                pattern: "[unclosed".to_string(),
                ..RegexSettings::default()
            };
            let mut cache = mut_cache();
            assert!(matches!(
                regex_replace("text", &invalid, &mut cache),
                Cow::Borrowed(_)
            ));
        });
    }

    /// 空パターンは元のテキストを返すようにすること
    #[test]
    fn test_empty_pattern_returns_original() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert!(matches!(
                regex_replace("text", &RegexSettings::default(), &mut cache),
                Cow::Borrowed(_)
            ));
        });
    }

    /// 大文字小文字を無視するフラグできるようにすること
    #[test]
    fn test_case_insensitive() {
        with_pool_test_lock(|| {
            let settings = RegexSettings {
                pattern: "abc".to_string(),
                replacement: "X".to_string(),
                case_insensitive: true,
                ..RegexSettings::default()
            };
            let mut cache = mut_cache();
            assert_eq!(regex_replace("AbC", &settings, &mut cache), "X");
        });
    }

    /// 同一設定ではコンパイル結果を再利用すること
    #[test]
    fn test_regex_cache_reuses_compiled_pattern() {
        let settings = settings(r"\d+");
        let mut cache = mut_cache();

        let first = cache.get_or_compile(&settings).map(std::ptr::from_ref);
        let second = cache.get_or_compile(&settings).map(std::ptr::from_ref);

        assert_eq!(first, second);
    }

    /// 設定変更時はキャッシュを無効化すること
    #[test]
    fn test_regex_cache_invalidates_on_settings_change() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();

            assert_eq!(regex_extract("a1b", &settings(r"\d"), &mut cache), "1");
            assert_eq!(
                regex_extract("a1b", &settings(r"[a-z]"), &mut cache),
                "a\nb"
            );
        });
    }

    /// プレビューが置換結果とエラーを返すようにすること
    #[test]
    fn test_preview_regex_replace() {
        with_pool_test_lock(|| {
            let ok = preview_regex_replace(
                REGEX_PREVIEW_SAMPLE,
                &RegexSettings {
                    pattern: r"\d+".to_string(),
                    replacement: "#".to_string(),
                    ..RegexSettings::default()
                },
            );
            assert!(ok.valid);
            assert_eq!(ok.result.as_deref(), Some("Hello world #"));

            let invalid = preview_regex_replace(
                REGEX_PREVIEW_SAMPLE,
                &RegexSettings {
                    pattern: "[unclosed".to_string(),
                    ..RegexSettings::default()
                },
            );
            assert!(!invalid.valid);
            assert!(invalid.result.is_none());
        });
    }

    /// 制限時間内に終わらないマッチはタイムアウトすること
    #[test]
    fn test_run_with_match_timeout_expires() {
        with_pool_test_lock(|| {
            let result = run_with_match_timeout_duration(Duration::from_millis(30), |cancel| {
                // キャンセルを見つつ長めに占有し、タイムアウト後は早期終了する
                let deadline = Instant::now() + Duration::from_millis(300);
                while Instant::now() < deadline {
                    check_cancel(cancel)?;
                    thread::sleep(Duration::from_millis(10));
                }
                Ok("done")
            });
            assert_eq!(result, Err(RegexMatchTimeout));
        });
    }

    /// 制限時間内に終わるマッチは結果を返すこと
    #[test]
    fn test_run_with_match_timeout_completes() {
        with_pool_test_lock(|| {
            let result = run_with_match_timeout_duration(Duration::from_millis(500), |_cancel| {
                thread::sleep(Duration::from_millis(10));
                Ok(42)
            });
            assert_eq!(result, Ok(42));
        });
    }

    /// タイムアウト時は置換せず元テキストを返すこと
    #[test]
    fn test_regex_replace_timeout_keeps_original() {
        with_pool_test_lock(|| {
            // ヘルパー経由で短い制限時間を検証し、本番パスは on_match_timeout と同じ挙動
            let text = "keep-me";
            let timed_out = run_with_match_timeout_duration(Duration::from_millis(20), |cancel| {
                let deadline = Instant::now() + Duration::from_millis(200);
                while Instant::now() < deadline {
                    check_cancel(cancel)?;
                    thread::sleep(Duration::from_millis(10));
                }
                Ok("replaced".to_string())
            });
            assert!(timed_out.is_err());
            assert!(matches!(on_match_timeout(text), Cow::Borrowed("keep-me")));
        });
    }

    /// タイムアウト後はキャンセルでスロットを速やかに解放すること
    #[test]
    fn test_timeout_cancels_and_releases_slot_promptly() {
        with_pool_test_lock(|| {
            let started = Instant::now();
            let result = run_with_match_timeout_duration(Duration::from_millis(40), |cancel| {
                // キャンセル無視なら 1 秒近く残る処理を、要求後はすぐ抜ける
                for _ in 0..100 {
                    check_cancel(cancel)?;
                    thread::sleep(Duration::from_millis(10));
                }
                Ok("done")
            });
            assert_eq!(result, Err(RegexMatchTimeout));

            wait_until_pool_idle();
            assert!(
                started.elapsed() < Duration::from_millis(300),
                "キャンセル後はスロットが速やかに戻ること (elapsed={:?})",
                started.elapsed()
            );
        });
    }

    /// プール飽和時は追加マッチを拒否し、解放後は再実行できること
    #[test]
    fn test_match_pool_rejects_when_saturated_then_recovers() {
        with_pool_test_lock(|| {
            let occupy = Duration::from_millis(400);
            let short = Duration::from_millis(40);

            // 上限までスロットを占有する (キャンセルを無視して飽和ウィンドウを確保)
            for _ in 0..REGEX_MATCH_MAX_IN_FLIGHT {
                let result = run_with_match_timeout_duration(short, move |_cancel| {
                    thread::sleep(occupy);
                    Ok("occupied")
                });
                assert_eq!(result, Err(RegexMatchTimeout));
            }

            assert_eq!(REGEX_MATCH_POOL.in_flight(), REGEX_MATCH_MAX_IN_FLIGHT);

            // 空きがなければ速やかにタイムアウトし、追加 spawn しない
            let started = Instant::now();
            let rejected = run_with_match_timeout_duration(Duration::from_millis(50), |_cancel| {
                thread::sleep(Duration::from_millis(10));
                Ok("should-not-run")
            });
            assert_eq!(rejected, Err(RegexMatchTimeout));
            assert!(started.elapsed() < Duration::from_millis(200));

            // スロット解放後は再び成功する
            wait_until_pool_idle();
            let recovered =
                run_with_match_timeout_duration(Duration::from_millis(200), |_cancel| Ok(7));
            assert_eq!(recovered, Ok(7));
        });
    }

    /// 置換ループがキャンセル要求で途中終了すること
    #[test]
    fn test_replace_all_cancellable_stops_on_cancel() {
        let re = Regex::new(r"\d").expect("pattern");
        let text = "a1b2c3d4e5";
        let cancel = MatchCancel::new();
        cancel.cancel();
        let result = replace_all_cancellable(&re, text, "X", &cancel);
        assert_eq!(result, Err(RegexMatchTimeout));
    }
}
