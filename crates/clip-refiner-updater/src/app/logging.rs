//! 更新プログラムのログ

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Mutex;

use tracing::Level;

// ======================================================================
// 定数
// ======================================================================
/// 更新プログラムのログファイル名
const LOG_FILE_NAME: &str = "updater.log";
/// ログファイルの上限 (1 MiB)。超えていれば起動時に作り直す
const MAX_LOG_BYTES: u64 = 1024 * 1024;

// ======================================================================
// 初期化
// ======================================================================
/// ログの出力先を用意する
///
/// ログを書けなくても更新は続けられるため、失敗しても戻り値では知らせない
///
/// # Arguments
/// * `log_dir` - ログの出力先フォルダ
pub(super) fn init(log_dir: &Path) {
    let Ok(file) = open_log_file(log_dir) else {
        return;
    };
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .try_init();
}

/// ログファイルを追記で開く。上限を超えていれば空にする
fn open_log_file(log_dir: &Path) -> io::Result<File> {
    fs::create_dir_all(log_dir)?;
    let path = log_dir.join(LOG_FILE_NAME);
    let oversized = fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES);
    if oversized {
        return File::create(path);
    }
    OpenOptions::new().create(true).append(true).open(path)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// 上限までは追記し、超えたら作り直すこと
    #[test]
    fn log_file_is_recreated_over_limit() {
        let dir = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-log-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(LOG_FILE_NAME);

        for expected in [3, 6] {
            open_log_file(&dir)
                .and_then(|mut file| file.write_all(b"abc"))
                .expect("append");
            assert_eq!(fs::metadata(&path).expect("meta").len(), expected);
        }

        let oversized = vec![b'x'; usize::try_from(MAX_LOG_BYTES).expect("size") + 1];
        fs::write(&path, oversized).expect("oversized");
        drop(open_log_file(&dir).expect("reopen"));
        assert_eq!(fs::metadata(&path).expect("meta").len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
