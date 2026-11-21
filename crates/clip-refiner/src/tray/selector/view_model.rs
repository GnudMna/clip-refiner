use std::io::Cursor;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageBuffer, ImageFormat};

use crate::config::{AppConfig, RegisteredClip, ResolvedClip};
use crate::consts;
use crate::refiner::RefineMode;
use crate::security::looks_sensitive;
use crate::tray::state::ClipRef;

// ======================================================================
// セレクター表示モデル
// ======================================================================

/// クイックセレクターで「自動変換中」として示す変換モードを返す
///
/// # Arguments
/// * `config` - 参照する設定
///
/// # Returns
/// * `Option<RefineMode>` - 自動変換が有効なら `mode`、無効なら `None`
pub(crate) fn quick_selector_current_mode(config: &AppConfig) -> Option<RefineMode> {
    config.auto_refine_enabled.then_some(config.mode)
}

/// クイックセレクター向けの変換モード JSON 配列を生成する
///
/// # Arguments
/// * `config` - 変換元の設定
///
/// # Returns
/// * `String` - お気に入り順を含む変換モードの JSON 配列文字列
pub(crate) fn quick_selector_json(config: &AppConfig) -> String {
    RefineMode::to_json_list(&config.favorite_modes)
}

/// クイックセレクター向けの変換モード一覧を JSON 値として生成する
///
/// # Arguments
/// * `config` - 変換元の設定
///
/// # Returns
/// * `serde_json::Value` - 変換モードの JSON 配列
pub(crate) fn quick_selector_value(config: &AppConfig) -> serde_json::Value {
    serde_json::from_str(&quick_selector_json(config)).unwrap_or_else(|_| serde_json::json!([]))
}

/// 登録クリップセレクター向けの JSON 配列を生成する
///
/// # Arguments
/// * `config` - 変換元の設定
///
/// # Returns
/// * `String` - 登録クリップの JSON 配列文字列
pub(crate) fn clip_selector_json(config: &AppConfig) -> String {
    clip_selector_value(config).to_string()
}

/// 登録クリップセレクター向けの一覧を JSON 値として生成する
///
/// テキスト件はキャッシュ済み `preview` のみを使い、本文は復号しない
///
/// # Arguments
/// * `config` - 変換元の設定
///
/// # Returns
/// * `serde_json::Value` - 登録クリップの JSON 配列
pub(crate) fn clip_selector_value(config: &AppConfig) -> serde_json::Value {
    serde_json::Value::Array(
        config
            .clips
            .iter()
            .enumerate()
            .map(|(index, clip)| clip_selector_item(config, index, clip))
            .collect(),
    )
}

/// 登録クリップ本文をホバー表示用に整える
///
/// 機密情報らしい本文と空白のみの本文は表示しない。長い本文は
/// `REGISTERED_CLIP_HOVER_TEXT_MAX_CHARS` 文字で切り詰め、末尾に `...` を付ける
///
/// # Arguments
/// * `text` - 復号した本文
///
/// # Returns
/// * `Option<String>` - 表示できる場合は整形後の本文、表示しない場合は `None`
pub(crate) fn clip_hover_text(text: &str) -> Option<String> {
    if text.trim().is_empty() || looks_sensitive(text) {
        return None;
    }

    let max_chars = consts::REGISTERED_CLIP_HOVER_TEXT_MAX_CHARS;
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => Some(format!("{}...", &text[..end])),
        None => Some(text.to_string()),
    }
}

// ======================================================================
// プライベート関数
// ======================================================================

/// 登録クリップ1件をセレクター向け JSON オブジェクトへ変換する
fn clip_selector_item(
    config: &AppConfig,
    index: usize,
    clip: &RegisteredClip,
) -> serde_json::Value {
    let preview = if clip.is_image() {
        clip.label.as_str()
    } else {
        clip.preview.as_str()
    };
    let mut value = serde_json::json!({
        "id": index.to_string(),
        "label": clip.label,
        "preview": preview,
        "kind": if clip.is_image() { "image" } else { "text" },
        "template": clip.template,
        "counter": clip.counter,
        "token": ClipRef::format_token(clip.identity_token()),
    });

    if clip.is_image()
        && let Some(image) = registered_image(config, index)
    {
        if let Some(thumbnail) = encode_image_preview_data_url(
            &image,
            consts::SELECTOR_IMAGE_PREVIEW_MAX_DIMENSION,
            consts::MAX_SELECTOR_IMAGE_PREVIEW_BYTES,
        ) {
            value["thumbnail"] = serde_json::Value::String(thumbnail);
        }
        if let Some(hover_preview) = encode_image_preview_data_url(
            &image,
            consts::SELECTOR_IMAGE_HOVER_PREVIEW_MAX_DIMENSION,
            consts::MAX_SELECTOR_IMAGE_HOVER_PREVIEW_BYTES,
        ) {
            value["hover_preview"] = serde_json::Value::String(hover_preview);
        }
    }

    value
}

/// 指定インデックスの登録画像を表示用画像へ変換する
fn registered_image(config: &AppConfig, index: usize) -> Option<DynamicImage> {
    let ResolvedClip::Image {
        width,
        height,
        rgba,
    } = config.resolve_registered_clip(index)?
    else {
        return None;
    };
    ImageBuffer::from_raw(width, height, rgba).map(DynamicImage::ImageRgba8)
}

/// 画像を縮小して JPEG Data URL へ変換する
fn encode_image_preview_data_url(
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

    /// お気に入り順をクイックセレクターの JSON に反映すること
    #[test]
    fn quick_selector_value_contains_favorite_order() {
        let config = AppConfig {
            favorite_modes: vec![RefineMode::Trim, RefineMode::UrlDecode],
            ..AppConfig::default()
        };

        let items = quick_selector_value(&config);
        let trim = items
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item["id"] == serde_json::json!("Trim"))
            })
            .expect("Trim");

        assert_eq!(trim["favoriteIndex"], 0);
    }

    /// 登録テキストの暗号文をセレクターの JSON に含めないこと
    #[test]
    fn clip_selector_value_excludes_ciphertext() {
        let config = AppConfig {
            clips: vec![RegisteredClip {
                label: "表示名".to_string(),
                preview: "プレビュー".to_string(),
                text_ciphertext: vec![1, 2, 3],
                image_file: None,
                template: false,
                counter: 3,
            }],
            ..AppConfig::default()
        };

        let item = &clip_selector_value(&config)[0];

        assert_eq!(item["id"], "0");
        assert_eq!(item["label"], "表示名");
        assert_eq!(item["preview"], "プレビュー");
        assert_eq!(item["kind"], "text");
        assert_eq!(item["template"], false);
        assert_eq!(item["counter"], 3);
        assert!(item.get("text_ciphertext").is_none());
    }

    /// 登録画像はラベルをプレビューとして表示すること
    #[test]
    fn clip_selector_value_uses_image_label_as_preview() {
        let config = AppConfig {
            clips: vec![RegisteredClip {
                label: "[画像] 2×2".to_string(),
                preview: String::new(),
                text_ciphertext: Vec::new(),
                image_file: Some("missing.enc".to_string()),
                template: false,
                counter: 0,
            }],
            ..AppConfig::default()
        };

        let item = &clip_selector_value(&config)[0];

        assert_eq!(item["preview"], "[画像] 2×2");
        assert_eq!(item["kind"], "image");
        assert!(item.get("thumbnail").is_none());
        assert!(item.get("hover_preview").is_none());
    }

    /// 通常の本文はそのままホバー表示用に返すこと
    #[test]
    fn clip_hover_text_keeps_plain_text() {
        let text = "1 行目\n2 行目 with  spaces";
        assert_eq!(clip_hover_text(text).as_deref(), Some(text));
    }

    /// 機密情報らしい本文と空白のみの本文はホバー表示しないこと
    #[test]
    fn clip_hover_text_hides_sensitive_and_blank() {
        assert_eq!(clip_hover_text("api_key=supersecret"), None);
        assert_eq!(clip_hover_text("  \n\t"), None);
    }

    /// 上限を超える本文は文字境界で切り詰めて `...` を付けること
    #[test]
    fn clip_hover_text_truncates_at_char_boundary() {
        let max = consts::REGISTERED_CLIP_HOVER_TEXT_MAX_CHARS;

        let exact = "あ".repeat(max);
        assert_eq!(clip_hover_text(&exact).as_deref(), Some(exact.as_str()));

        let over = "あ".repeat(max + 1);
        let truncated = clip_hover_text(&over).expect("truncated");
        assert_eq!(truncated, format!("{}...", "あ".repeat(max)));
    }

    /// 画像プレビューを JPEG Data URL へ変換すること
    #[test]
    fn image_preview_starts_with_jpeg_data_url_prefix() {
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let image = ImageBuffer::from_raw(2, 2, rgba)
            .map(DynamicImage::ImageRgba8)
            .expect("image");

        let data_url =
            encode_image_preview_data_url(&image, 64, consts::MAX_SELECTOR_IMAGE_PREVIEW_BYTES)
                .expect("thumbnail");

        assert!(data_url.starts_with("data:image/jpeg;base64,"));
    }
}
