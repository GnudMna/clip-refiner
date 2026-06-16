use std::fs;
use std::path::PathBuf;
use std::sync::{
    OnceLock,
    atomic::{AtomicI64, Ordering},
};

use anyhow::{Context, Result};
use chrono::NaiveDate;

// ======================================================================
// ロガートレイト
// ======================================================================
/// アプリケーション全体のロガー用トレイト
///
/// 異なるバックエンド（tracing, mockなど）を抽象化するための共通インターフェースを提供します。
pub trait Logger: Send + Sync {
    /// 情報ログ（INFOレベル）を出力する
    fn info(&self, msg: &str);
    /// 警告ログ（WARNレベル）を出力する
    fn warn(&self, msg: &str);
    /// エラーログ（ERRORレベル）を出力する
    fn error(&self, msg: &str);
    /// デバッグログ（DEBUGレベル）を出力する
    #[allow(unused)]
    fn debug(&self, msg: &str);
}

// ======================================================================
// tracing ロガー
// ======================================================================
/// tracing クレートを使用した Logger の実装
///
/// ファイルへのログ出力と、定期的な古いログのクリーンアップ機能を備えています。
pub struct TracingLogger {
    log_dir: PathBuf,
    /// 最後にクリーンアップを実行した日をUNIXエポックからの日数で保持する。
    /// -1 は「まだ実行していない」を示す。
    /// Mutex の代わりに AtomicI64 を使うことで、ログ呼び出しごとの
    /// ロック取得コストをなくし、日付が変わっていない場合は完全にロックフリーで動作する。
    last_cleanup_day: AtomicI64,
}

impl TracingLogger {
    /// 新しい `TracingLogger` インスタンスを生成する
    ///
    /// # Arguments
    /// * `log_dir` - ログファイルを保存するディレクトリのパス
    ///
    /// # Returns
    /// * `Self` - 生成された `TracingLogger` インスタンス。
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            last_cleanup_day: AtomicI64::new(-1),
        }
    }

    fn check_and_cleanup(&self) {
        let today = chrono::Local::now()
            .date_naive()
            .signed_duration_since(
                NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 は常に有効な日付です"),
            )
            .num_days();

        // ファストパス: 今日のクリーンアップが既に完了していればロックなしで即リターン
        if self.last_cleanup_day.load(Ordering::Relaxed) >= today {
            return;
        }

        // compare_exchange で1スレッドだけがクリーンアップを実行する
        // 失敗した場合は他スレッドが先に更新済みなので何もしない
        if self
            .last_cleanup_day
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |prev| {
                if prev < today { Some(today) } else { None }
            })
            .is_ok()
            && let Err(e) = cleanup_old_logs(&self.log_dir, 14)
        {
            // ここで自身を呼び出すと無限ループになる可能性があるため、tracing を直接使う
            tracing::warn!("自動ログクリーンアップに失敗: {:?}", e);
        }
    }
}

impl Logger for TracingLogger {
    fn info(&self, msg: &str) {
        self.check_and_cleanup();
        tracing::info!("{}", msg);
    }
    fn warn(&self, msg: &str) {
        self.check_and_cleanup();
        tracing::warn!("{}", msg);
    }
    fn error(&self, msg: &str) {
        self.check_and_cleanup();
        tracing::error!("{}", msg);
    }
    fn debug(&self, msg: &str) {
        self.check_and_cleanup();
        tracing::debug!("{}", msg);
    }
}

// ======================================================================
// グローバルロガー
// ======================================================================
/// グローバルなロガーインスタンス
static GLOBAL_LOGGER: OnceLock<Box<dyn Logger>> = OnceLock::new();

/// グローバルロガーを初期化する
///
/// アプリケーション起動時に一度だけ呼び出し、グローバルなロガーインスタンスを設定します。
///
/// # Arguments
/// * `logger` - 使用するロガーの実装（`Box<dyn Logger>`）
pub fn init_global_logger(logger: Box<dyn Logger>) {
    let _ = GLOBAL_LOGGER.set(logger);
}

// ======================================================================
// ログクリーンアップ
// ======================================================================
/// 指定された日数より古いログファイルを削除する
///
/// 指定されたディレクトリ内の古いログファイルをスキャンし、期限を過ぎたものを削除します。
///
/// # Arguments
/// * `log_dir` - ログファイルが格納されているディレクトリのパス
/// * `max_days` - ログを保持する最大日数
///
/// # Returns
/// * `Result<()>` - クリーンアップが成功した場合は `Ok(())`、失敗した場合は `Err` を返します。
pub fn cleanup_old_logs(log_dir: &std::path::Path, max_days: i64) -> Result<()> {
    let now = chrono::Local::now().date_naive();
    let entries = fs::read_dir(log_dir).context("ログディレクトリの読み取りに失敗")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(filename) = path.file_name().and_then(|n| n.to_str())
        {
            // clip-refiner.log.YYYY-MM-DD 形式を想定
            if let Some(date_str) = filename.strip_prefix("clip-refiner.log.")
                && let Ok(file_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            {
                let age = now - file_date;
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
// ロガー取得
// ======================================================================
/// グローバルロガーを取得する
///
/// 初期化されていない場合は、何もしない `NoOpLogger` を返します。
///
/// # Returns
/// * `&'static dyn Logger` - 現在設定されているグローバルロガーへの参照。
pub fn get_logger() -> &'static dyn Logger {
    GLOBAL_LOGGER
        .get()
        .map(|b| b.as_ref())
        .unwrap_or(&NoOpLogger)
}

/// ロガーが未初期化の場合のフォールバック用
struct NoOpLogger;
impl Logger for NoOpLogger {
    fn info(&self, _msg: &str) {}
    fn warn(&self, _msg: &str) {}
    fn error(&self, _msg: &str) {}
    fn debug(&self, _msg: &str) {}
}

// ======================================================================
// ログマクロ
// ======================================================================
/// 情報ログ（INFOレベル）を出力するマクロ
///
/// `format!` 構文をサポートしており、グローバルロガー経由で出力されます。
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().info(&format!($($arg)*));
    };
}

/// 警告ログ（WARNレベル）を出力するマクロ
///
/// `format!` 構文をサポートしており、グローバルロガー経由で出力されます。
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().warn(&format!($($arg)*));
    };
}

/// エラーログ（ERRORレベル）を出力するマクロ
///
/// `format!` 構文をサポートしており、グローバルロガー経由で出力されます。
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().error(&format!($($arg)*));
    };
}

/// デバッグログ（DEBUGレベル）を出力するマクロ
///
/// `format!` 構文をサポートしており、グローバルロガー経由で出力されます。
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().debug(&format!($($arg)*));
    };
}
// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockLogger {
        logs: Mutex<Vec<String>>,
    }

    impl Logger for MockLogger {
        fn info(&self, msg: &str) {
            self.logs
                .lock()
                .expect("MockLogger ロックの取得に失敗")
                .push(format!("INFO: {}", msg));
        }
        fn warn(&self, msg: &str) {
            self.logs
                .lock()
                .expect("MockLogger ロックの取得に失敗")
                .push(format!("WARN: {}", msg));
        }
        fn error(&self, msg: &str) {
            self.logs
                .lock()
                .expect("MockLogger ロックの取得に失敗")
                .push(format!("ERROR: {}", msg));
        }
        fn debug(&self, msg: &str) {
            self.logs
                .lock()
                .expect("MockLogger ロックの取得に失敗")
                .push(format!("DEBUG: {}", msg));
        }
    }

    #[test]
    fn test_mock_logger() {
        let logger = MockLogger {
            logs: Mutex::new(Vec::new()),
        };
        logger.info("test info");
        logger.error("test error");

        let logs = logger.logs.lock().expect("MockLogger ロックの取得に失敗");
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], "INFO: test info");
        assert_eq!(logs[1], "ERROR: test error");
    }
}
