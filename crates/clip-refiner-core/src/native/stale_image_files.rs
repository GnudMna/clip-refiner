//! 以前の版が一時フォルダへ書き出した、貼り付け用の画像ファイルの削除
//!
//! 現在は画像をディスクへ書き出さない (`clipboard_file`)。残った平文の PNG だけを対象とする

use std::fs;
use std::path::{Path, PathBuf};

// ======================================================================
// 定数
// ======================================================================
/// 一時フォルダ直下にある、以前の版の書き出し先フォルダ名の階層
const IMAGE_DIR: [&str; 2] = ["ClipRefiner", "clipboard-images"];

/// 以前の版が付けていたファイル名の日時の書式と拡張子
const FILE_STAMP_FORMAT: &str = "%Y%m%d-%H%M%S";
const FILE_EXTENSION: &str = "png";

/// さらに以前の版が付けていたファイル名の接頭辞
const LEGACY_FILE_PREFIX: &str = "ClipRefiner-";

// ======================================================================
// パブリック関数
// ======================================================================
/// 以前の版が書き出した画像ファイルをすべて削除する
///
/// 書き出し先フォルダが無い場合は何もせず、削除できないファイルは残す
pub(crate) fn remove_stale_image_files() {
    let dir = IMAGE_DIR
        .iter()
        .fold(std::env::temp_dir(), |dir, name| dir.join(name));
    remove_own_files(&dir);
    // 親フォルダも空なら削除する。ほかの内容があれば失敗し、そのまま残る
    if let Some(parent) = dir.parent() {
        let _ = fs::remove_dir(parent);
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// フォルダ内の、このアプリが書き出した画像ファイルを削除する
///
/// 空になったフォルダも削除する。削除に失敗したものは残し、次回に再試行する
fn remove_own_files(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let own_files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_own_file(path))
        .collect();
    for path in own_files {
        let _ = fs::remove_file(path);
    }
    // 空でなければ失敗し、ほかのファイルを残す
    let _ = fs::remove_dir(dir);
}

/// このアプリが書き出した画像ファイルか
fn is_own_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == FILE_EXTENSION)
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(is_own_file_stem)
}

/// このアプリが書き出した画像ファイルの名前 (拡張子を除く) か
///
/// `<日時>` または `<日時>-<連番>` の形を対象とし、接頭辞付きの名前も含める
fn is_own_file_stem(stem: &str) -> bool {
    let stem = stem.strip_prefix(LEGACY_FILE_PREFIX).unwrap_or(stem);
    // 日時部分は `YYYYMMDD-HHMMSS` の 15 文字
    let Some((stamp, serial)) = stem.split_at_checked(15) else {
        return false;
    };
    let serial_ok = serial.is_empty()
        || serial
            .strip_prefix('-')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    serial_ok && jiff::civil::DateTime::strptime(FILE_STAMP_FORMAT, stamp).is_ok()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn unique_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!("clip-refiner-{label}-{nanos}"))
    }

    /// 書き出した時期によらず、このアプリの画像ファイルだけを削除すること
    #[test]
    fn remove_own_files_keeps_foreign_files() {
        let dir = unique_dir("stale-image-files");
        fs::create_dir_all(&dir).expect("dir");
        let own = [
            "20261113-120000.png",
            "20261113-120000-1.png",
            "ClipRefiner-20260101-000000-1.png",
        ];
        let foreign = ["photo.png", "20260101-000000.txt"];
        for name in own.iter().chain(&foreign) {
            fs::write(dir.join(name), b"x").expect("write");
        }

        remove_own_files(&dir);

        for name in own {
            assert!(!dir.join(name).exists(), "{name}");
        }
        for name in foreign {
            assert!(dir.join(name).exists(), "{name}");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// このアプリの画像ファイルだけが入ったフォルダは、フォルダごと削除すること
    #[test]
    fn remove_own_files_removes_emptied_dir() {
        let dir = unique_dir("stale-image-dir");
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("20261113-120000.png"), b"x").expect("write");

        remove_own_files(&dir);

        assert!(!dir.exists());
    }

    /// 日時 (と連番) の名前と以前の版の名前だけを自アプリのファイルと判定すること
    #[test]
    fn is_own_file_stem_matches_stamp_names_only() {
        assert!(is_own_file_stem("20261113-120000"));
        assert!(is_own_file_stem("20261113-120000-12"));
        assert!(is_own_file_stem("ClipRefiner-20261113-120000"));

        assert!(!is_own_file_stem("photo"));
        assert!(!is_own_file_stem("20261113-120000-"));
        assert!(!is_own_file_stem("20261113-120000-a"));
        assert!(!is_own_file_stem("20261399-120000"));
        assert!(!is_own_file_stem("Other-20261113-120000"));
    }

    /// フォルダが存在しなくても削除処理が失敗しないこと
    #[test]
    fn remove_own_files_ignores_missing_dir() {
        remove_own_files(&unique_dir("stale-image-missing"));
    }
}
