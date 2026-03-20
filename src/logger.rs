use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// アプリケーション全体のロガー用トレイト
pub trait Logger: Send + Sync {
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
    #[allow(unused)]
    fn debug(&self, msg: &str);
}

/// tracing クレートを使用した Logger の実装
pub struct TracingLogger {
    log_dir: PathBuf,
    last_cleanup: Mutex<Option<NaiveDate>>,
}

impl TracingLogger {
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            last_cleanup: Mutex::new(None),
        }
    }

    fn check_and_cleanup(&self) {
        let now = chrono::Local::now().date_naive();
        let mut last_cleanup = self.last_cleanup.lock().unwrap();

        if last_cleanup.map_or(true, |date| now > date) {
            if let Err(e) = cleanup_old_logs(&self.log_dir, 14) {
                // ここで自身を呼び出すと無限ループになる可能性があるため、tracing を直接使う
                tracing::warn!("自動ログクリーンアップに失敗: {:?}", e);
            }
            *last_cleanup = Some(now);
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

/// グローバルなロガーインスタンス
static GLOBAL_LOGGER: OnceLock<Box<dyn Logger>> = OnceLock::new();

/// グローバルロガーを初期化する
pub fn init_global_logger(logger: Box<dyn Logger>) {
    let _ = GLOBAL_LOGGER.set(logger);
}

/// 指定された日数より古いログファイルを削除する
pub fn cleanup_old_logs(log_dir: &std::path::Path, max_days: i64) -> Result<()> {
    let now = chrono::Local::now().date_naive();
    let entries = std::fs::read_dir(log_dir).context("ログディレクトリの読み取りに失敗")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                // clip-refiner.log.YYYY-MM-DD 形式を想定
                if filename.starts_with("clip-refiner.log.") {
                    let date_str = &filename["clip-refiner.log.".len()..];
                    if let Ok(file_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        let age = now - file_date;
                        if age.num_days() > max_days {
                            tracing::info!("古いログファイルを削除します: {}", filename);
                            let _ = std::fs::remove_file(path);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// グローバルロガーを取得する
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

/// ログ出力用マクロ（利用を簡潔にするため）
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().info(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().warn(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().error(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logger::get_logger().debug(&format!($($arg)*));
    };
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockLogger {
        logs: Mutex<Vec<String>>,
    }

    impl Logger for MockLogger {
        fn info(&self, msg: &str) {
            self.logs.lock().unwrap().push(format!("INFO: {}", msg));
        }
        fn warn(&self, msg: &str) {
            self.logs.lock().unwrap().push(format!("WARN: {}", msg));
        }
        fn error(&self, msg: &str) {
            self.logs.lock().unwrap().push(format!("ERROR: {}", msg));
        }
        fn debug(&self, msg: &str) {
            self.logs.lock().unwrap().push(format!("DEBUG: {}", msg));
        }
    }

    #[test]
    fn test_mock_logger() {
        let logger = MockLogger {
            logs: Mutex::new(Vec::new()),
        };
        logger.info("test info");
        logger.error("test error");

        let logs = logger.logs.lock().unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], "INFO: test info");
        assert_eq!(logs[1], "ERROR: test error");
    }
}
