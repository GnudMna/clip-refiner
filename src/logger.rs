use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;

// ======================================================================
// 起動時初期化
// ======================================================================
/// ログディレクトリの古いファイルを削除する
///
/// アプリケーション起動時に一度呼び出す
///
/// # Arguments
/// * `log_dir` - ログファイルが格納されているディレクトリのパス
pub fn cleanup_on_startup(log_dir: &Path) {
    if let Err(e) = cleanup_old_logs(log_dir, 14) {
        tracing::warn!("起動時ログクリーンアップに失敗: {:?}", e);
    }
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
/// * `max_days` - ログを保持する最大日数
///
/// # Returns
/// * `Result<()>` - クリーンアップが成功した場合は `Ok(())`、失敗した場合は `Err` を返す
pub fn cleanup_old_logs(log_dir: &Path, max_days: i64) -> Result<()> {
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
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 保持期間を超えたログファイルが削除されること
    #[test]
    fn test_cleanup_old_logs_removes_expired_files() {
        let base = std::env::temp_dir().join(format!(
            "clip-refiner-log-cleanup-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("UNIX_EPOCH より前の時刻")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("一時ディレクトリの作成に失敗");

        let old_name = "clip-refiner.log.2000-01-01";
        let recent_name = "clip-refiner.log.2099-01-01";
        fs::write(base.join(old_name), "old").expect("古いログファイルの作成に失敗");
        fs::write(base.join(recent_name), "recent").expect("新しいログファイルの作成に失敗");

        cleanup_old_logs(&base, 14).expect("ログクリーンアップに失敗");

        assert!(!base.join(old_name).exists(), "古いログが残っている");
        assert!(base.join(recent_name).exists(), "新しいログが削除された");

        let _ = fs::remove_dir_all(&base);
    }
}
