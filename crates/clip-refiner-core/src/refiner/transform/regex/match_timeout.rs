use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::consts::{REGEX_MATCH_MAX_IN_FLIGHT, REGEX_MATCH_TIMEOUT_MS};

use regex::Regex;
use tracing::warn;

// ======================================================================
// マッチ実行タイムアウト
// ======================================================================
/// 正規表現マッチ操作のタイムアウト
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RegexMatchTimeout;

/// タイムアウト時に実行側へ中断を伝えるフラグ
///
/// `regex` クレートは単一の `find` 実行中を打ち切れないため、マッチ境界での
/// 協調キャンセルに使う
pub(super) struct MatchCancel {
    cancelled: AtomicBool,
}

impl MatchCancel {
    /// 未キャンセル状態で生成する
    pub(super) fn new() -> Self {
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
/// タイムアウト時はキャンセルを要求し、実行側がマッチ境界で中断してスロットを返す
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
    /// * `Result<RegexMatchPermit, RegexMatchTimeout>` - 取得成功時は `Permit`
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
/// 制限時間を超えた場合はキャンセルを要求して `Err(RegexMatchTimeout)` を返す
/// 実行側は `MatchCancel` を見てマッチ境界で中断する。同時実行は
/// `REGEX_MATCH_MAX_IN_FLIGHT` で上限する
pub(super) fn run_with_match_timeout<T, F>(op: F) -> Result<T, RegexMatchTimeout>
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
pub(super) fn run_with_match_timeout_duration<T, F>(
    timeout: Duration,
    op: F,
) -> Result<T, RegexMatchTimeout>
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
pub(super) fn check_cancel(cancel: &MatchCancel) -> Result<(), RegexMatchTimeout> {
    if cancel.is_cancelled() {
        Err(RegexMatchTimeout)
    } else {
        Ok(())
    }
}

/// キャンセル可能な置換を実行する
pub(super) fn replace_all_cancellable(
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
        // 全体マッチ (グループ 0) は常に存在するため、到達しない分岐
        let Some(m) = caps.get(0) else {
            continue;
        };
        out.push_str(&text[last..m.start()]);
        caps.expand(replacement, &mut out);
        last = m.end();
    }
    check_cancel(cancel)?;
    out.push_str(&text[last..]);
    Ok(out)
}

/// キャンセル可能な抽出を実行する
pub(super) fn extract_cancellable(
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
pub(super) fn split_cancellable(
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
pub(super) fn on_match_timeout(text: &str) -> Cow<'_, str> {
    warn!(
        timeout_ms = REGEX_MATCH_TIMEOUT_MS,
        in_flight = REGEX_MATCH_POOL.in_flight(),
        max_in_flight = REGEX_MATCH_MAX_IN_FLIGHT,
        "正規表現の実行がタイムアウトしたため元のテキストを維持する"
    );
    Cow::Borrowed(text)
}

// ======================================================================
// テスト用ヘルパー (グローバルプール直列化)
// ======================================================================
/// グローバルプールを触るテストの直列化用
#[cfg(test)]
static POOL_TEST_LOCK: Mutex<()> = Mutex::new(());

/// プールが空くまで待ってからクロージャを実行し、終了後も空くまで待つ
#[cfg(test)]
pub(super) fn with_pool_test_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = POOL_TEST_LOCK.lock().expect("regex pool test lock");
    wait_until_pool_idle();
    let result = f();
    wait_until_pool_idle();
    result
}

/// 実行中スロットが 0 になるまで短周期で待つ
#[cfg(test)]
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{
        MatchCancel, REGEX_MATCH_POOL, RegexMatchTimeout, check_cancel, on_match_timeout,
        replace_all_cancellable, run_with_match_timeout_duration, wait_until_pool_idle,
        with_pool_test_lock,
    };
    use crate::consts::REGEX_MATCH_MAX_IN_FLIGHT;

    use regex::Regex;

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

    /// 制限時間内に終わるマッチは結果を返すようにすること
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

    /// タイムアウト時は置換せず元テキストを返すようにすること
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

    /// プール飽和時は追加マッチを拒否し、解放後は再実行できるようにすること
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
