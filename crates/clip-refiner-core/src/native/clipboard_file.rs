//! 画像をエクスプローラへファイルとして貼り付けられるようにする
//!
//! PNG を一時フォルダへ書き出し、そのパスを `CF_HDROP` としてクリップボードへ追加する
//! 画像のビットマップ (`arboard`) を書き込んだ直後に呼び、既存の形式は消さない

use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use image::{ImageBuffer, ImageFormat, RgbaImage};

use crate::consts;

// ======================================================================
// 定数
// ======================================================================
/// 一時フォルダ直下に作る、書き出し先フォルダ名の階層
const IMAGE_DIR: [&str; 2] = ["ClipRefiner", "clipboard-images"];

/// 書き出すファイル名の日時の書式と拡張子 (削除対象の判定にも使う)
const FILE_STAMP_FORMAT: &str = "%Y%m%d-%H%M%S";
const FILE_EXTENSION: &str = "png";

/// 以前の版が付けていたファイル名の接頭辞 (残ったファイルを削除対象に含めるために使う)
const LEGACY_FILE_PREFIX: &str = "ClipRefiner-";

/// クリップボードを開く最大試行回数
const OPEN_CLIPBOARD_ATTEMPTS: usize = 10;

// ======================================================================
// パブリック関数
// ======================================================================
/// RGBA 画像を PNG へエンコードする
///
/// # Arguments
/// * `width` - 幅 (ピクセル)
/// * `height` - 高さ (ピクセル)
/// * `rgba` - RGBA ピクセル列
///
/// # Returns
/// * `io::Result<Vec<u8>>` - PNG のバイト列
///
/// # Errors
/// バッファ長が寸法と合わない場合、またはエンコードに失敗した場合
pub(crate) fn encode_png(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let image: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| io::Error::other("RGBA バッファの長さが画像の寸法と一致しない"))?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(io::Error::other)?;
    Ok(png)
}

/// 保持時間を過ぎた、このアプリが書き出した画像ファイルを削除する
///
/// 書き出し先フォルダが無い場合は何もせず、削除できないファイルは残す
pub(crate) fn remove_stale_image_files() {
    let retention = Duration::from_secs(consts::CLIPBOARD_IMAGE_FILE_RETENTION_SECS);
    remove_stale_files(&image_dir(), retention, SystemTime::now());
}

/// PNG を一時フォルダへ書き出し、そのパスを `CF_HDROP` としてクリップボードへ追加する
///
/// 既存のクリップボード内容 (ビットマップなど) は消さない
/// 書き出す前に、保持時間を過ぎた古い画像ファイルを削除する
///
/// # Arguments
/// * `png` - 書き出す PNG のバイト列
///
/// # Returns
/// * `io::Result<PathBuf>` - 書き出したファイルのパス
///
/// # Errors
/// ファイルの書き出し、またはクリップボードへの追加に失敗した場合
pub(crate) fn attach_png_file(png: &[u8]) -> io::Result<PathBuf> {
    remove_stale_image_files();
    let path = write_png_file(&image_dir(), png)?;

    let path_str = path
        .to_str()
        .ok_or_else(|| io::Error::other("ファイルパスが UTF-8 ではない"))?;
    let _clipboard = clipboard_win::Clipboard::new_attempts(OPEN_CLIPBOARD_ATTEMPTS)
        .map_err(|e| io::Error::other(format!("クリップボードを開けない: {e}")))?;
    clipboard_win::raw::set_file_list(&[path_str])
        .map_err(|e| io::Error::other(format!("ファイル一覧を書き込めない: {e}")))?;
    Ok(path)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 画像ファイルの書き出し先フォルダ
fn image_dir() -> PathBuf {
    IMAGE_DIR
        .iter()
        .fold(std::env::temp_dir(), |dir, name| dir.join(name))
}

/// フォルダを作り、日時の名前で PNG を書き出す
///
/// 同名のファイルがある場合は上書きせず、連番を付けた名前にする
fn write_png_file(dir: &Path, png: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;

    let stamp = jiff::ZonedDateTime::now().strftime(FILE_STAMP_FORMAT);
    for serial in 0..100 {
        let name = if serial == 0 {
            format!("{stamp}.{FILE_EXTENSION}")
        } else {
            format!("{stamp}-{serial}.{FILE_EXTENSION}")
        };
        let path = dir.join(name);

        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        io::Write::write_all(&mut file, png)?;
        return Ok(path);
    }
    Err(io::Error::other("画像ファイルの名前を決められない"))
}

/// 保持時間を過ぎた、このアプリが書き出した画像ファイルを削除する
///
/// 削除に失敗したファイルは残し、次回に再試行する
fn remove_stale_files(dir: &Path, retention: Duration, now: SystemTime) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_ours = path.extension().is_some_and(|ext| ext == FILE_EXTENSION)
            && path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(is_own_file_stem);
        if !is_ours {
            continue;
        }

        let age = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok());
        if age.is_some_and(|age| age >= retention) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// このアプリが書き出した画像ファイルの名前 (拡張子を除く) か
///
/// `<日時>` または `<日時>-<連番>` の形を対象とし、以前の版の接頭辞付きの名前も含める
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
    use super::*;

    fn unique_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!("clip-refiner-{label}-{nanos}"))
    }

    /// RGBA を PNG にエンコードでき、寸法の合わないバッファは拒否すること
    #[test]
    fn encode_png_roundtrips_and_rejects_bad_length() {
        let rgba = vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let png = encode_png(2, 2, &rgba).expect("encode");
        assert_eq!(&png[..4], b"\x89PNG");

        let decoded = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(decoded.into_raw(), rgba);

        assert!(encode_png(2, 2, &[0; 4]).is_err());
    }

    /// PNG をアプリ名を含まない日時の名前で書き出し、同名があれば連番にして上書きしないこと
    #[test]
    fn write_png_file_never_overwrites() {
        let dir = unique_dir("clipboard-file-write");
        let first = write_png_file(&dir, b"first").expect("first");
        let second = write_png_file(&dir, b"second").expect("second");

        assert_ne!(first, second);
        assert_eq!(fs::read(&first).expect("read"), b"first");
        assert_eq!(fs::read(&second).expect("read"), b"second");
        let name = first.file_name().and_then(|n| n.to_str()).expect("name");
        assert!(!name.contains("ClipRefiner"));
        assert!(is_own_file_stem(
            first.file_stem().and_then(|n| n.to_str()).expect("stem")
        ));
        assert_eq!(first.extension().and_then(|e| e.to_str()), Some("png"));

        let _ = fs::remove_dir_all(&dir);
    }

    /// 保持時間を過ぎたこのアプリの画像ファイルだけを削除すること
    #[test]
    fn remove_stale_files_only_removes_expired_own_files() {
        let dir = unique_dir("clipboard-file-sweep");
        let fresh = write_png_file(&dir, b"fresh").expect("fresh");
        let stale = write_png_file(&dir, b"stale").expect("stale");
        let foreign_png = dir.join("photo.png");
        let foreign_text = dir.join("20260101-000000.txt");
        let legacy = dir.join(format!("{LEGACY_FILE_PREFIX}20260101-000000-1.png"));
        fs::write(&foreign_png, b"x").expect("foreign png");
        fs::write(&foreign_text, b"x").expect("foreign text");
        fs::write(&legacy, b"x").expect("legacy");

        let retention = Duration::from_mins(1);
        // `stale` は保持時間を過ぎた時点、`fresh` は保持時間内として扱う
        let modified = |path: &Path| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .expect("mtime")
        };
        fs::File::options()
            .write(true)
            .open(&fresh)
            .and_then(|f| f.set_modified(SystemTime::now()))
            .expect("touch fresh");
        fs::File::options()
            .write(true)
            .open(&stale)
            .and_then(|f| f.set_modified(modified(&stale) - Duration::from_mins(2)))
            .expect("age stale");
        for path in [&foreign_png, &foreign_text, &legacy] {
            fs::File::options()
                .write(true)
                .open(path)
                .and_then(|f| f.set_modified(modified(path) - Duration::from_mins(2)))
                .expect("age other");
        }

        remove_stale_files(&dir, retention, SystemTime::now());

        assert!(fresh.exists());
        assert!(!stale.exists());
        // 以前の版の接頭辞付きの名前も削除し、形の違う名前は残す
        assert!(!legacy.exists());
        assert!(foreign_png.exists());
        assert!(foreign_text.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    /// 実クリップボードへビットマップとファイルの両方を載せるようにすること
    ///
    /// 実クリップボードの内容を書き換えるため、手動実行 (`cargo test -- --ignored`) 専用
    #[test]
    #[ignore = "実クリップボードを書き換える"]
    fn set_image_as_file_puts_bitmap_and_file_on_clipboard() {
        use clipboard_win::{formats, is_format_avail};

        use crate::refiner::text_clipboard::ImageClipboard;

        let rgba = vec![200; 4 * 4 * 4];
        let mut clipboard = arboard::Clipboard::new().expect("clipboard");
        ImageClipboard::set_image_as_file(&mut clipboard, 4, 4, rgba).expect("write");

        assert!(is_format_avail(formats::CF_DIBV5), "ビットマップが無い");
        assert!(is_format_avail(formats::CF_HDROP), "ファイルが無い");

        let files = clipboard.get().file_list().expect("file list");
        assert_eq!(files.len(), 1);
        let png = fs::read(&files[0]).expect("read file");
        assert_eq!(&png[..4], b"\x89PNG");
        eprintln!("貼り付け用ファイル: {}", files[0].display());
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
    fn remove_stale_files_ignores_missing_dir() {
        remove_stale_files(
            &unique_dir("clipboard-file-missing"),
            Duration::ZERO,
            SystemTime::now(),
        );
    }
}
