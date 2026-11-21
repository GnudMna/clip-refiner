use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use super::clip_store_key::clip_store_key;
use super::paths::get_config_dir;
use super::permissions::{restrict_private_dir_permissions, restrict_private_file_permissions};

use crate::consts;
use crate::security::{decrypt_bytes, encrypt_bytes};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageBuffer, ImageFormat, RgbaImage};

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
pub fn registered_images_dir() -> Result<PathBuf> {
    let dir = get_config_dir()?.join(REGISTERED_IMAGES_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir).context("登録画像ディレクトリの作成に失敗しました")?;
        restrict_private_dir_permissions(&dir)?;
    }
    Ok(dir)
}

/// RGBA 画像を暗号化 PNG として保存し、設定に記録する相対ファイル名を返す
pub fn save_registered_image(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
    validate_rgba_buffer(width, height, rgba)?;

    let png = encode_png(width, height, rgba)?;
    if png.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
        bail!("登録画像の PNG サイズが上限を超えています");
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &png)?;
    if encrypted.len() > consts::MAX_REGISTERED_IMAGE_ENCRYPTED_BYTES {
        bail!("登録画像の暗号化サイズが上限を超えています");
    }

    let filename = format!(
        "{}.{ENCRYPTED_IMAGE_EXT}",
        blake3::hash(&encrypted).to_hex()
    );
    let path = registered_images_dir()?.join(&filename);
    if !path.exists() {
        fs::write(&path, &encrypted).context("登録画像ファイルの書き込みに失敗しました")?;
        restrict_private_file_permissions(&path)?;
    }

    Ok(filename)
}

/// 登録画像ファイルを RGBA バッファとして読み込む
pub fn load_registered_image(relative_filename: &str) -> Result<(u32, u32, Vec<u8>)> {
    let png = read_registered_image_png(relative_filename)?;
    decode_png(&png)
}

/// 登録画像ファイルが存在するかどうかを返す
pub fn registered_image_exists(relative_filename: &str) -> bool {
    resolve_registered_image_path(relative_filename)
        .ok()
        .is_some_and(|path| path.is_file())
}

/// 登録画像ファイルを削除する
pub fn delete_registered_image(relative_filename: &str) {
    if let Ok(path) = resolve_registered_image_path(relative_filename) {
        let _ = fs::remove_file(path);
    }
}

/// v2 の平文 PNG を暗号化ファイルへ移行する
///
/// 既に `.enc` の場合はファイル名をそのまま返す
pub fn migrate_plain_image_to_encrypted(relative_filename: &str) -> Result<String> {
    if relative_filename.ends_with(&format!(".{ENCRYPTED_IMAGE_EXT}")) {
        return Ok(relative_filename.to_string());
    }
    if !relative_filename.ends_with(&format!(".{LEGACY_IMAGE_EXT}")) {
        bail!("登録画像のファイル名が不正です");
    }

    let path = resolve_registered_image_path(relative_filename)?;
    let png =
        fs::read(&path).with_context(|| format!("登録画像の読み込みに失敗: {}", path.display()))?;
    if png.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
        bail!("登録画像の PNG サイズが上限を超えています");
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &png)?;
    let filename = format!(
        "{}.{ENCRYPTED_IMAGE_EXT}",
        blake3::hash(&encrypted).to_hex()
    );
    let new_path = registered_images_dir()?.join(&filename);
    if !new_path.exists() {
        fs::write(&new_path, &encrypted).context("登録画像の暗号化保存に失敗しました")?;
        restrict_private_file_permissions(&new_path)?;
    }
    let _ = fs::remove_file(path);
    Ok(filename)
}

/// 登録画像の表示用ラベルを生成する
pub fn format_registered_image_label(width: u32, height: u32) -> String {
    format!("[画像] {width}×{height}")
}

/// セレクター表示用の JPEG サムネイル Data URL を生成する
///
/// 生成に失敗した場合やサイズ上限を超える場合は `None` を返す
pub fn registered_image_thumbnail_data_url(
    relative_filename: &str,
    max_dimension: u32,
) -> Option<String> {
    registered_image_preview_data_url(
        relative_filename,
        max_dimension,
        consts::MAX_SELECTOR_IMAGE_PREVIEW_BYTES,
    )
}

/// セレクター hover プレビュー用の JPEG Data URL を生成する
pub fn registered_image_hover_preview_data_url(
    relative_filename: &str,
    max_dimension: u32,
) -> Option<String> {
    registered_image_preview_data_url(
        relative_filename,
        max_dimension,
        consts::MAX_SELECTOR_IMAGE_HOVER_PREVIEW_BYTES,
    )
}

/// セレクター表示用の JPEG Data URL を生成する
fn registered_image_preview_data_url(
    relative_filename: &str,
    max_dimension: u32,
    max_bytes: usize,
) -> Option<String> {
    let png = read_registered_image_png(relative_filename).ok()?;
    let image = image::load_from_memory_with_format(&png, ImageFormat::Png).ok()?;
    encode_thumbnail_data_url(&image, max_dimension, max_bytes)
}

// ======================================================================
// プライベート関数
// ======================================================================
fn read_registered_image_png(relative_filename: &str) -> Result<Vec<u8>> {
    let path = resolve_registered_image_path(relative_filename)?;
    let bytes =
        fs::read(&path).with_context(|| format!("登録画像の読み込みに失敗: {}", path.display()))?;

    if relative_filename.ends_with(&format!(".{ENCRYPTED_IMAGE_EXT}")) {
        if bytes.len() > consts::MAX_REGISTERED_IMAGE_ENCRYPTED_BYTES {
            bail!("登録画像の暗号化サイズが上限を超えています");
        }
        let key = clip_store_key()?;
        let png = decrypt_bytes(&key, &bytes)?;
        if png.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
            bail!("登録画像の PNG サイズが上限を超えています");
        }
        return Ok(png);
    }

    if bytes.len() > consts::MAX_REGISTERED_IMAGE_BYTES {
        bail!("登録画像の PNG サイズが上限を超えています");
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
        bail!("登録画像のファイル名が不正です");
    }

    let path = registered_images_dir()?.join(relative_filename);
    if !path.starts_with(registered_images_dir()?) {
        bail!("登録画像のファイル名が不正です");
    }

    Ok(path)
}

fn is_allowed_image_filename(filename: &str) -> bool {
    filename.ends_with(&format!(".{ENCRYPTED_IMAGE_EXT}"))
        || filename.ends_with(&format!(".{LEGACY_IMAGE_EXT}"))
}

fn validate_rgba_buffer(width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if width == 0 || height == 0 {
        bail!("登録画像のサイズが不正です");
    }
    if width > consts::MAX_REGISTERED_IMAGE_DIMENSION
        || height > consts::MAX_REGISTERED_IMAGE_DIMENSION
    {
        bail!("登録画像の辺長が上限を超えています");
    }

    let pixels = u64::from(width) * u64::from(height);
    if pixels > consts::MAX_REGISTERED_IMAGE_PIXELS {
        bail!("登録画像のピクセル数が上限を超えています");
    }

    let expected_len = usize::try_from(pixels)
        .context("登録画像のピクセル数が大きすぎます")?
        .saturating_mul(4);
    if rgba.len() != expected_len {
        bail!("登録画像の RGBA バッファ長が不正です");
    }

    Ok(())
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let image: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .context("登録画像の RGBA バッファから画像を構築できませんでした")?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .context("登録画像の PNG エンコードに失敗しました")?;
    Ok(png)
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .context("登録画像の PNG デコードに失敗しました")?;
    let rgba = image.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());

    if width == 0
        || height == 0
        || width > consts::MAX_REGISTERED_IMAGE_DIMENSION
        || height > consts::MAX_REGISTERED_IMAGE_DIMENSION
    {
        bail!("登録画像のサイズが不正です");
    }

    let pixels = u64::from(width) * u64::from(height);
    if pixels > consts::MAX_REGISTERED_IMAGE_PIXELS {
        bail!("登録画像のピクセル数が上限を超えています");
    }

    Ok((width, height, rgba.into_raw()))
}

fn encode_thumbnail_data_url(
    image: &DynamicImage,
    max_dimension: u32,
    max_bytes: usize,
) -> Option<String> {
    let thumbnail = image.thumbnail(max_dimension, max_dimension);
    let mut jpeg = Vec::new();
    thumbnail
        .write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
        .ok()?;
    if jpeg.is_empty() || jpeg.len() > max_bytes {
        return None;
    }

    Some(format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)))
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

    /// PNG の往復で元の RGBA を復元できること
    #[test]
    fn png_roundtrip_preserves_rgba() {
        let rgba = vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let png = encode_png(2, 2, &rgba).expect("encode");
        let (width, height, decoded) = decode_png(&png).expect("decode");
        assert_eq!((width, height), (2, 2));
        assert_eq!(decoded, rgba);
    }

    /// サムネイル Data URL を生成できること
    #[test]
    fn thumbnail_data_url_starts_with_jpeg_prefix() {
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_raw(2, 2, rgba).expect("image"));
        let data_url =
            encode_thumbnail_data_url(&image, 64, consts::MAX_SELECTOR_IMAGE_PREVIEW_BYTES)
                .expect("thumbnail");
        assert!(data_url.starts_with("data:image/jpeg;base64,"));
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
