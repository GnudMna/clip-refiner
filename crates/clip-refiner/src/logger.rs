use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};

// ======================================================================
// 定数
// ======================================================================
/// ログファイル名のプレフィックス
pub const LOG_FILE_PREFIX: &str = "clip-refiner.log";
/// ログファイルを保持する最大日数
const LOG_RETENTION_DAYS: i64 = 14;

// ======================================================================
// 起動時初期化
// ======================================================================
/// ログディレクトリの古いファイルを削除する
///
/// アプリケーション起動時に一度呼び出す。常駐中は日付切り替え時に Appender が再実行する
///
/// # Arguments
/// * `log_dir` - ログファイルが格納されているディレクトリのパス
pub fn cleanup_on_startup(log_dir: &Path) {
    let today = Local::now().date_naive();
    if let Err(e) = cleanup_old_logs(log_dir, LOG_FILE_PREFIX, today, LOG_RETENTION_DAYS) {
        tracing::warn!("起動時ログクリーンアップに失敗: {:?}", e);
    }
}

// ======================================================================
// ローカル日付の日次ファイル Appender
// ======================================================================
/// ローカルタイムゾーンの日付で日次ローテーションするファイル Appender
///
/// ファイル名は `{prefix}.YYYY-MM-DD` (例: `clip-refiner.log.2026-07-12`)
/// `tracing_appender::non_blocking` と組み合わせて使う
pub struct LocalDailyFileAppender {
    directory: PathBuf,
    file_name_prefix: String,
    /// 現在日付の取得 (本番は `Local::now().date_naive()`、テストでは差し替え)
    date_fn: Box<dyn Fn() -> NaiveDate + Send + Sync>,
    state: Mutex<AppenderState>,
}

struct AppenderState {
    date: NaiveDate,
    file: File,
}

impl LocalDailyFileAppender {
    /// ローカル日付で日次ローテーションする Appender を作成する
    ///
    /// # Arguments
    /// * `directory` - ログファイルを置くディレクトリ
    /// * `file_name_prefix` - ファイル名のプレフィックス (例: `clip-refiner.log`)
    ///
    /// # Returns
    /// * `Result<Self>` - 作成に成功した場合は Appender、失敗時はエラー
    pub fn new(directory: impl AsRef<Path>, file_name_prefix: impl Into<String>) -> Result<Self> {
        Self::with_date_fn(directory, file_name_prefix, || Local::now().date_naive())
    }

    /// 日付取得関数を差し替えて Appender を作成する (テスト用)
    ///
    /// # Arguments
    /// * `directory` - ログファイルを置くディレクトリ
    /// * `file_name_prefix` - ファイル名のプレフィックス
    /// * `date_fn` - 現在のローカル日付を返す関数
    ///
    /// # Returns
    /// * `Result<Self>` - 作成に成功した場合は Appender、失敗時はエラー
    pub fn with_date_fn(
        directory: impl AsRef<Path>,
        file_name_prefix: impl Into<String>,
        date_fn: impl Fn() -> NaiveDate + Send + Sync + 'static,
    ) -> Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        let file_name_prefix = file_name_prefix.into();
        let date = date_fn();
        let file = open_log_file(&directory, &file_name_prefix, date)
            .context("ログファイルのオープンに失敗")?;

        Ok(Self {
            directory,
            file_name_prefix,
            date_fn: Box::new(date_fn),
            state: Mutex::new(AppenderState { date, file }),
        })
    }

    /// 内部状態のロックを取得する
    ///
    /// 別スレッドのパニックでロックが poison されていても、書き込みを止めないよう中身をそのまま使う
    ///
    /// # Arguments
    /// * `&self` - ロック対象の Appender
    ///
    /// # Returns
    /// * `MutexGuard<'_, AppenderState>` - 内部状態のガード
    fn lock_state(&self) -> MutexGuard<'_, AppenderState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 必要なら日付に合わせてファイルを切り替える
    ///
    /// 切り替えた場合は、常駐中も古いログが溜まらないよう保持期間を超えたファイルを削除する
    ///
    /// # Arguments
    /// * `state` - 現在開いているファイルの状態
    ///
    /// # Returns
    /// * `io::Result<()>` - 切替成功時は `Ok(())`
    fn rotate_if_needed(&self, state: &mut AppenderState) -> io::Result<()> {
        let today = (self.date_fn)();
        if state.date == today {
            return Ok(());
        }
        state.file = open_log_file(&self.directory, &self.file_name_prefix, today)?;
        state.date = today;
        if let Err(e) = cleanup_old_logs(
            &self.directory,
            &self.file_name_prefix,
            today,
            LOG_RETENTION_DAYS,
        ) {
            tracing::warn!("日付切り替え時のログクリーンアップに失敗: {:?}", e);
        }
        Ok(())
    }
}

impl Write for LocalDailyFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}

impl Write for &LocalDailyFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.lock_state();
        self.rotate_if_needed(&mut state)?;
        state.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.lock_state();
        self.rotate_if_needed(&mut state)?;
        state.file.flush()
    }
}

impl LocalDailyFileAppender {
    /// 非同期キューを経由せず、記録を直ちにファイルへ書き込む
    ///
    /// パニックフックから呼ぶ前提で、ロック中の同スレッドからの再入でデッドロックしないよう
    /// `try_lock` を使う。ロックを取れない場合は書き込まずにエラーを返す
    ///
    /// # Arguments
    /// * `&self` - 書き込み先の Appender
    /// * `record` - 書き込む記録 (末尾の改行を含む)
    ///
    /// # Returns
    /// * `io::Result<()>` - 書き込みとフラッシュに成功した場合は `Ok(())`
    pub fn write_immediately(&self, record: &str) -> io::Result<()> {
        let mut state = match self.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "ログ Appender が使用中",
                ));
            }
        };
        self.rotate_if_needed(&mut state)?;
        state.file.write_all(record.as_bytes())?;
        state.file.flush()
    }
}

/// `Arc` で共有した [`LocalDailyFileAppender`] への書き込み口
///
/// `tracing_appender::non_blocking` に渡しつつ、パニックフックからも同じ Appender へ書き込むために使う
pub struct SharedLogAppender(pub Arc<LocalDailyFileAppender>);

impl Write for SharedLogAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

/// 指定日付のログファイルを追記モードで開く
///
/// # Arguments
/// * `directory` - ログディレクトリ
/// * `prefix` - ファイル名プレフィックス
/// * `date` - ローカル日付
///
/// # Returns
/// * `io::Result<File>` - オープンしたファイル
fn open_log_file(directory: &Path, prefix: &str, date: NaiveDate) -> io::Result<File> {
    let path = directory.join(format!("{prefix}.{date}"));
    OpenOptions::new().create(true).append(true).open(path)
}

// ======================================================================
// ログクリーンアップ
// ======================================================================
/// 指定された日数より古いログファイルを削除する
///
/// 指定されたディレクトリ内の古いログファイルをスキャンし、期限を過ぎたものを削除する
///
/// # Arguments
/// * `log_dir` - ログファイルが格納されているディレクトリのパス
/// * `prefix` - ログファイル名のプレフィックス (`{prefix}.YYYY-MM-DD` 形式を対象にする)
/// * `today` - 経過日数の基準にする現在のローカル日付
/// * `max_days` - ログを保持する最大日数
///
/// # Returns
/// * `Result<()>` - クリーンアップが成功した場合は `Ok(())`、失敗した場合は `Err` を返す
pub fn cleanup_old_logs(
    log_dir: &Path,
    prefix: &str,
    today: NaiveDate,
    max_days: i64,
) -> Result<()> {
    let entries = fs::read_dir(log_dir).context("ログディレクトリの読み取りに失敗")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(filename) = path.file_name().and_then(|n| n.to_str())
        {
            // {prefix}.YYYY-MM-DD 形式を想定
            if let Some(date_str) = filename
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('.'))
                && let Ok(file_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            {
                let age = today - file_date;
                if age.num_days() > max_days {
                    tracing::info!("古いログファイルを削除します: {}", filename);
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
    Ok(())
}

// ======================================================================
// ログマクロ
// ======================================================================
/// 情報ログ(INFOレベル)を出力するマクロ
///
/// `format!` 構文をサポートし、`tracing` 経由で出力される
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*);
    };
}

/// 警告ログ(WARNレベル)を出力するマクロ
///
/// `format!` 構文をサポートし、`tracing` 経由で出力される
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*);
    };
}

/// エラーログ(ERRORレベル)を出力するマクロ
///
/// `format!` 構文をサポートし、`tracing` 経由で出力される
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*);
    };
}

/// デバッグログ(DEBUGレベル)を出力するマクロ
///
/// `format!` 構文をサポートし、`tracing` 経由で出力される
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*);
    };
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use chrono::NaiveDate;

    use super::*;

    /// テスト用の一時ディレクトリを作成する
    fn temp_log_dir(label: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "clip-refiner-log-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("UNIX_EPOCH より前の時刻")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("一時ディレクトリの作成に失敗");
        base
    }

    /// 保持期間を超えたログファイルが削除されるようにすること
    #[test]
    fn test_cleanup_old_logs_removes_expired_files() {
        let base = temp_log_dir("cleanup");

        let old_name = "clip-refiner.log.2000-01-01";
        let recent_name = "clip-refiner.log.2099-01-01";
        fs::write(base.join(old_name), "old").expect("古いログファイルの作成に失敗");
        fs::write(base.join(recent_name), "recent").expect("新しいログファイルの作成に失敗");

        let today = NaiveDate::from_ymd_opt(2026, 7, 12).expect("有効な日付");
        cleanup_old_logs(&base, "clip-refiner.log", today, 14).expect("ログクリーンアップに失敗");

        assert!(!base.join(old_name).exists(), "古いログが残っている");
        assert!(base.join(recent_name).exists(), "新しいログが削除された");

        let _ = fs::remove_dir_all(&base);
    }

    /// 日付切り替えでファイルを切り替えたとき、保持期間を超えたログを削除すること
    ///
    /// 常駐中は起動時クリーンアップが走らないため、ローテーション契機で行う
    #[test]
    fn rotation_removes_expired_logs() {
        let base = temp_log_dir("rotate-cleanup");
        let day0 = NaiveDate::from_ymd_opt(2026, 7, 12).expect("有効な日付");
        let day1 = NaiveDate::from_ymd_opt(2026, 7, 13).expect("有効な日付");
        let step = Arc::new(AtomicU8::new(0));
        let step_for_fn = Arc::clone(&step);

        let expired = "clip-refiner.log.2026-06-01";
        let retained = "clip-refiner.log.2026-07-05";
        fs::write(base.join(expired), "expired").expect("期限切れログの作成に失敗");
        fs::write(base.join(retained), "retained").expect("保持対象ログの作成に失敗");

        let mut appender =
            LocalDailyFileAppender::with_date_fn(&base, "clip-refiner.log", move || {
                if step_for_fn.load(Ordering::SeqCst) == 0 {
                    day0
                } else {
                    day1
                }
            })
            .expect("Appender の作成に失敗");

        // 同日中は削除しない
        appender
            .write_all(b"day0\n")
            .expect("1日目の書き込みに失敗");
        assert!(base.join(expired).exists(), "同日中に削除された");

        step.store(1, Ordering::SeqCst);
        appender
            .write_all(b"day1\n")
            .expect("2日目の書き込みに失敗");

        assert!(!base.join(expired).exists(), "期限切れログが残っている");
        assert!(base.join(retained).exists(), "保持対象ログが削除された");

        let _ = fs::remove_dir_all(&base);
    }

    /// ロックが poison されても書き込みと flush に成功すること
    #[test]
    fn write_recovers_from_poisoned_lock() {
        let base = temp_log_dir("poisoned");
        let day = NaiveDate::from_ymd_opt(2026, 7, 12).expect("有効な日付");
        let appender = Arc::new(
            LocalDailyFileAppender::with_date_fn(&base, "clip-refiner.log", move || day)
                .expect("Appender の作成に失敗"),
        );

        // ロック保持中にパニックさせて poison する
        let poisoner = Arc::clone(&appender);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.state.lock().expect("ロックの取得に失敗");
            panic!("poison 用のパニック");
        })
        .join();
        assert!(appender.state.is_poisoned(), "poison されていない");

        let mut shared = SharedLogAppender(Arc::clone(&appender));
        shared
            .write_all(b"after\n")
            .expect("poison 後の書き込みに失敗");
        shared.flush().expect("poison 後の flush に失敗");

        let content = fs::read_to_string(base.join("clip-refiner.log.2026-07-12"))
            .expect("ログの読み取りに失敗");
        assert_eq!(content, "after\n");

        let _ = fs::remove_dir_all(&base);
    }

    /// 共有した Appender への通常書き込みと即時書き込みが同じファイルに残ること
    #[test]
    fn shared_appender_and_immediate_write_use_same_file() {
        let base = temp_log_dir("shared");
        let day = NaiveDate::from_ymd_opt(2026, 7, 12).expect("有効な日付");
        let appender = Arc::new(
            LocalDailyFileAppender::with_date_fn(&base, "clip-refiner.log", move || day)
                .expect("Appender の作成に失敗"),
        );

        // 非同期キュー経由を想定した書き込み口
        let mut shared = SharedLogAppender(Arc::clone(&appender));
        shared
            .write_all(
                b"queued
",
            )
            .expect("共有書き込みに失敗");
        shared.flush().expect("flush に失敗");

        // パニックフックを想定した即時書き込み
        appender
            .write_immediately(
                "panic record
",
            )
            .expect("即時書き込みに失敗");

        let content = fs::read_to_string(base.join("clip-refiner.log.2026-07-12"))
            .expect("ログの読み取りに失敗");
        assert_eq!(
            content,
            "queued
panic record
"
        );

        let _ = fs::remove_dir_all(&base);
    }

    /// 同じスレッドがロック中でも即時書き込みがデッドロックせずエラーを返すこと
    #[test]
    fn immediate_write_does_not_block_while_locked() {
        let base = temp_log_dir("immediate-locked");
        let appender =
            LocalDailyFileAppender::new(&base, "clip-refiner.log").expect("Appender の作成に失敗");

        let held = appender.state.lock().expect("ロックの取得に失敗");
        let err = appender
            .write_immediately(
                "panic record
",
            )
            .expect_err("ロック中は書き込まないこと");
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);

        drop(held);
        let _ = fs::remove_dir_all(&base);
    }

    /// 同一ローカル日付では同じログファイルへ追記すること
    #[test]
    fn local_daily_appender_appends_same_day() {
        let base = temp_log_dir("same-day");
        let day = NaiveDate::from_ymd_opt(2026, 7, 12).expect("有効な日付");

        let mut appender =
            LocalDailyFileAppender::with_date_fn(&base, "clip-refiner.log", move || day)
                .expect("Appender の作成に失敗");

        appender
            .write_all(b"first\n")
            .expect("1回目の書き込みに失敗");
        appender
            .write_all(b"second\n")
            .expect("2回目の書き込みに失敗");
        appender.flush().expect("flush に失敗");

        let path = base.join("clip-refiner.log.2026-07-12");
        assert!(path.exists(), "当日のログファイルが無い");
        let content = fs::read_to_string(&path).expect("ログの読み取りに失敗");
        assert_eq!(content, "first\nsecond\n");

        // 他日付のファイルは作られない
        assert!(!base.join("clip-refiner.log.2026-07-13").exists());

        let _ = fs::remove_dir_all(&base);
    }

    /// ローカル日付が変わると新しいログファイルへ切り替えるようにすること
    #[test]
    fn local_daily_appender_rotates_on_date_change() {
        let base = temp_log_dir("rotate");
        let day0 = NaiveDate::from_ymd_opt(2026, 7, 12).expect("有効な日付");
        let day1 = NaiveDate::from_ymd_opt(2026, 7, 13).expect("有効な日付");
        let step = Arc::new(AtomicU8::new(0));
        let step_for_fn = Arc::clone(&step);

        let mut appender =
            LocalDailyFileAppender::with_date_fn(&base, "clip-refiner.log", move || {
                if step_for_fn.load(Ordering::SeqCst) == 0 {
                    day0
                } else {
                    day1
                }
            })
            .expect("Appender の作成に失敗");

        appender
            .write_all(b"day0\n")
            .expect("1日目の書き込みに失敗");
        appender.flush().expect("1日目の flush に失敗");

        step.store(1, Ordering::SeqCst);

        appender
            .write_all(b"day1\n")
            .expect("2日目の書き込みに失敗");
        appender.flush().expect("2日目の flush に失敗");

        let path0 = base.join("clip-refiner.log.2026-07-12");
        let path1 = base.join("clip-refiner.log.2026-07-13");
        assert_eq!(
            fs::read_to_string(&path0).expect("1日目ログの読み取りに失敗"),
            "day0\n"
        );
        assert_eq!(
            fs::read_to_string(&path1).expect("2日目ログの読み取りに失敗"),
            "day1\n"
        );

        let _ = fs::remove_dir_all(&base);
    }
}
