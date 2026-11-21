use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use image::{ImageBuffer, ImageFormat, RgbaImage};

use super::clip_store_key::clip_store_key;
use super::paths::get_config_dir;
use super::permissions::{restrict_private_dir_permissions, restrict_private_file_permissions};
use crate::consts;
use crate::error::ConfigError;
use crate::security::{decrypt_bytes, encrypt_bytes};

type Result<T> = std::result::Result<T, ConfigError>;

fn clips_err(msg: impl Into<String>) -> ConfigError {
    ConfigError::RegisteredClipsSave(msg.into())
}

// ======================================================================
// 定数
// ======================================================================
const REGISTERED_IMAGES_DIR: &str = "registered-images";
const ENCRYPTED_IMAGE_EXT: &str = "enc";
const LEGACY_IMAGE_EXT: &str = "png";

// ======================================================================
// パブリック関数
// ======================================================================
/// 登録画像の保存ディレクトリを取得する
///
/// 存在しない場合は作成し、所有者のみアクセス可能なパーミッションを設定する
///
/// # Returns
/// * `Result<PathBuf>` - 登録画像ディレクトリのパス
///
/// # Errors
/// 設定ディレクトリ取得またはディレクトリ作成に失敗した場合 (パーミッション設定の失敗は警告のみ)
pub fn registered_images_dir() -> Result<PathBuf> {
    let dir = get_config_dir()?.join(REGISTERED_IMAGES_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| clips_err(format!("登録画像ディレクトリの作成に失敗しました: {e}")))?;
        if let Err(e) = restrict_private_dir_permissions(&dir) {
            tracing::warn!("登録画像ディレクトリのパーミッション設定に失敗: {:?}", e);
        }
    }
    Ok(dir)
}

/// RGBA 画像を暗号化 PNG として保存し、設定に記録する相対ファイル名を返す
///
/// # Arguments
/// * `width` - 画像幅 (ピクセル)
/// * `height` - 画像高さ (ピクセル)
/// * `rgba` - RGBA ピクセル列
///
/// # Returns
/// * `Result<String>` - `registered-images` 内の相対ファイル名
///
/// # Errors
/// バッファ検証、PNG エンコード、サイズ上限超過、鍵取得、暗号化、ファイル書き込みのいずれかに失敗した場合
pub fn save_registered_image(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
    validate_rgba_buffer(width, height, rgba)?;

    let png = encode_png(width, height, rgba)?;
    if png.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
        return Err(clips_err("登録画像の PNG サイズが上限を超えています"));
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &png)?;
    if encrypted.len() > consts::MAX_REGISTERED_IMAGE_ENCRYPTED_BYTES {
        return Err(clips_err("登録画像の暗号化サイズが上限を超えています"));
    }

    let filename = format!(
        "{}.{ENCRYPTED_IMAGE_EXT}",
        blake3::hash(&encrypted).to_hex()
    );
    let path = registered_images_dir()?.join(&filename);
    if !path.exists() {
        fs::write(&path, &encrypted)
            .map_err(|e| clips_err(format!("登録画像ファイルの書き込みに失敗しました: {e}")))?;
        if let Err(e) = restrict_private_file_permissions(&path) {
            tracing::warn!("登録画像ファイルのパーミッション設定に失敗: {:?}", e);
        }
    }

    Ok(filename)
}

/// 登録画像ファイルを RGBA バッファとして読み込む
///
/// # Arguments
/// * `relative_filename` - `registered-images` 内の相対ファイル名
///
/// # Returns
/// * `Result<(u32, u32, Vec<u8>)>` - 幅、高さ、RGBA ピクセル列
///
/// # Errors
/// ファイル名検証、読み込み、復号、PNG デコードのいずれかに失敗した場合
pub fn load_registered_image(relative_filename: &str) -> Result<(u32, u32, Vec<u8>)> {
    let png = read_registered_image_png(relative_filename)?;
    decode_png(&png)
}

/// 登録画像ファイルが存在するかどうかを返す
///
/// # Arguments
/// * `relative_filename` - `registered-images` 内の相対ファイル名
///
/// # Returns
/// * `bool` - ファイルが存在する場合は `true`
pub fn registered_image_exists(relative_filename: &str) -> bool {
    resolve_registered_image_path(relative_filename)
        .ok()
        .is_some_and(|path| path.is_file())
}

/// 登録画像ファイルを削除する
///
/// パス解決に失敗した場合やファイルが無い場合は何もしない
///
/// # Arguments
/// * `relative_filename` - `registered-images` 内の相対ファイル名
pub fn delete_registered_image(relative_filename: &str) {
    if let Ok(path) = resolve_registered_image_path(relative_filename) {
        let _ = fs::remove_file(path);
    }
}

/// v2 の平文 PNG を暗号化ファイルへ移行する
///
/// 既に `.enc` の場合はファイル名をそのまま返す
///
/// # Arguments
/// * `relative_filename` - 移行対象の相対ファイル名
///
/// # Returns
/// * `Result<String>` - 暗号化後の相対ファイル名
///
/// # Errors
/// ファイル名検証、読み込み、暗号化、書き込みのいずれかに失敗した場合
pub fn migrate_plain_image_to_encrypted(relative_filename: &str) -> Result<String> {
    if relative_filename.ends_with(&format!(".{ENCRYPTED_IMAGE_EXT}")) {
        return Ok(relative_filename.to_string());
    }
    if !relative_filename.ends_with(&format!(".{LEGACY_IMAGE_EXT}")) {
        return Err(clips_err("登録画像のファイル名が不正です"));
    }

    let path = resolve_registered_image_path(relative_filename)?;
    let png = fs::read(&path)
        .map_err(|e| clips_err(format!("登録画像の読み込みに失敗: {}: {e}", path.display())))?;
    if png.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
        return Err(clips_err("登録画像の PNG サイズが上限を超えています"));
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &png)?;
    let filename = format!(
        "{}.{ENCRYPTED_IMAGE_EXT}",
        blake3::hash(&encrypted).to_hex()
    );
    let new_path = registered_images_dir()?.join(&filename);
    if !new_path.exists() {
        fs::write(&new_path, &encrypted)
            .map_err(|e| clips_err(format!("登録画像の暗号化保存に失敗しました: {e}")))?;
        if let Err(e) = restrict_private_file_permissions(&new_path) {
            tracing::warn!("登録画像ファイルのパーミッション設定に失敗: {:?}", e);
        }
    }
    let _ = fs::remove_file(path);
    Ok(filename)
}

/// 登録画像の表示用ラベルを生成する
///
/// # Arguments
/// * `width` - 画像幅 (ピクセル)
/// * `height` - 画像高さ (ピクセル)
///
/// # Returns
/// * `String` - `[画像] {width}×{height}` 形式のラベル
pub fn format_registered_image_label(width: u32, height: u32) -> String {
    format!("[画像] {width}×{height}")
}

// ======================================================================
// プライベート関数
// ======================================================================
fn read_registered_image_png(relative_filename: &str) -> Result<Vec<u8>> {
    let path = resolve_registered_image_path(relative_filename)?;
    let bytes = fs::read(&path)
        .map_err(|e| clips_err(format!("登録画像の読み込みに失敗: {}: {e}", path.display())))?;

    if relative_filename.ends_with(&format!(".{ENCRYPTED_IMAGE_EXT}")) {
        if bytes.len() > consts::MAX_REGISTERED_IMAGE_ENCRYPTED_BYTES {
            return Err(clips_err("登録画像の暗号化サイズが上限を超えています"));
        }
        let key = clip_store_key()?;
        let png = decrypt_bytes(&key, &bytes)?;
        if png.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
            return Err(clips_err("登録画像の PNG サイズが上限を超えています"));
        }
        return Ok(png);
    }

    if bytes.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
        return Err(clips_err("登録画像の PNG サイズが上限を超えています"));
    }
    Ok(bytes)
}

fn resolve_registered_image_path(relative_filename: &str) -> Result<PathBuf> {
    if relative_filename.is_empty()
        || relative_filename.contains('/')
        || relative_filename.contains('\\')
        || relative_filename.contains("..")
        || !is_allowed_image_filename(relative_filename)
    {
        return Err(clips_err("登録画像のファイル名が不正です"));
    }

    let path = registered_images_dir()?.join(relative_filename);
    if !path.starts_with(registered_images_dir()?) {
        return Err(clips_err("登録画像のファイル名が不正です"));
    }

    Ok(path)
}

fn is_allowed_image_filename(filename: &str) -> bool {
    filename.ends_with(&format!(".{ENCRYPTED_IMAGE_EXT}"))
        || filename.ends_with(&format!(".{LEGACY_IMAGE_EXT}"))
}

fn validate_rgba_buffer(width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(clips_err("登録画像のサイズが不正です"));
    }
    if width > consts::MAX_REGISTERED_IMAGE_DIMENSION
        || height > consts::MAX_REGISTERED_IMAGE_DIMENSION
    {
        return Err(clips_err("登録画像の辺長が上限を超えています"));
    }

    let pixels = u64::from(width) * u64::from(height);
    if pixels > consts::MAX_REGISTERED_IMAGE_PIXELS {
        return Err(clips_err("登録画像のピクセル数が上限を超えています"));
    }

    let expected_len = usize::try_from(pixels)
        .map_err(|e| clips_err(format!("登録画像のピクセル数が大きすぎます: {e}")))?
        .saturating_mul(4);
    if rgba.len() != expected_len {
        return Err(clips_err("登録画像の RGBA バッファ長が不正です"));
    }

    Ok(())
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let image: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| clips_err("登録画像の RGBA バッファから画像を構築できませんでした"))?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| clips_err(format!("登録画像の PNG エンコードに失敗しました: {e}")))?;
    Ok(png)
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|e| clips_err(format!("登録画像の PNG デコードに失敗しました: {e}")))?;
    let rgba = image.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());

    if width == 0
        || height == 0
        || width > consts::MAX_REGISTERED_IMAGE_DIMENSION
        || height > consts::MAX_REGISTERED_IMAGE_DIMENSION
    {
        return Err(clips_err("登録画像のサイズが不正です"));
    }

    let pixels = u64::from(width) * u64::from(height);
    if pixels > consts::MAX_REGISTERED_IMAGE_PIXELS {
        return Err(clips_err("登録画像のピクセル数が上限を超えています"));
    }

    Ok((width, height, rgba.into_raw()))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// RGBA バッファ長が不正な場合は保存を拒否すること
    #[test]
    fn save_rejects_invalid_rgba_length() {
        let err = save_registered_image(2, 2, &[0; 8]).unwrap_err();
        assert!(err.to_string().contains("RGBA"));
    }

    /// PNG の往復で元の RGBA を復元できるようにすること
    #[test]
    fn png_roundtrip_preserves_rgba() {
        let rgba = vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let png = encode_png(2, 2, &rgba).expect("encode");
        let (width, height, decoded) = decode_png(&png).expect("decode");
        assert_eq!((width, height), (2, 2));
        assert_eq!(decoded, rgba);
    }

    /// 相対パス traversal を拒否すること
    #[test]
    fn resolve_rejects_path_traversal() {
        assert!(resolve_registered_image_path("../secret.png").is_err());
        assert!(resolve_registered_image_path("a/b.png").is_err());
    }

    /// 許可されていない拡張子を拒否すること
    #[test]
    fn resolve_rejects_unknown_extension() {
        assert!(resolve_registered_image_path("image.jpg").is_err());
    }
}
