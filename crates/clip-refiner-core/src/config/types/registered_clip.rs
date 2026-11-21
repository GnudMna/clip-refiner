use serde::{Deserialize, Serialize};

// ======================================================================
// 登録クリップ
// ======================================================================
/// クリップボードへコピーするための登録クリップ (テキストまたは画像)
///
/// `registered-clips.dat` に暗号化して保存される
/// `image_file` が指定されている場合は画像登録として扱う
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredClip {
    /// 一覧表示用のラベル
    pub label: String,
    /// クリップボードへコピーする本文 (画像登録時は空)
    #[serde(default)]
    pub text: String,
    /// 登録画像ファイル名 (`registered-images` ディレクトリ内の相対パス)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_file: Option<String>,
}

impl RegisteredClip {
    /// 画像登録かどうかを返す
    pub fn is_image(&self) -> bool {
        self.image_file.is_some()
    }

    /// 登録クリップセレクター向けの JSON オブジェクトを生成する
    pub fn to_clip_selector_json(
        &self,
        index: usize,
        preview: &str,
        thumbnail: Option<&str>,
        hover_preview: Option<&str>,
    ) -> serde_json::Value {
        let mut value = serde_json::json!({
            "id": index.to_string(),
            "label": self.label,
            "preview": preview,
            "kind": if self.is_image() { "image" } else { "text" },
        });
        if let Some(thumbnail) = thumbnail {
            value["thumbnail"] = serde_json::Value::String(thumbnail.to_string());
        }
        if let Some(hover_preview) = hover_preview {
            value["hover_preview"] = serde_json::Value::String(hover_preview.to_string());
        }
        value
    }
}

// ======================================================================
// 登録クリップボード内容
// ======================================================================
/// 登録済みクリップボード内容 (コピー用)
#[derive(Debug, Clone)]
pub enum ResolvedClip {
    /// テキスト
    Text(String),
    /// RGBA 画像
    Image {
        /// 画像幅 (ピクセル)
        width: u32,
        /// 画像高さ (ピクセル)
        height: u32,
        /// RGBA ピクセル列
        rgba: Vec<u8>,
    },
}

// ======================================================================
// 登録クリップの追加
// ======================================================================
/// 登録クリップの追加に失敗した理由
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddRegisteredClipError {
    /// 空文字または空白のみ
    #[error("登録クリップが空です")]
    Empty,
    /// クリップボード上限を超える
    #[error("登録クリップのサイズが上限を超えています")]
    TooLarge,
    /// 登録件数の上限に達している
    #[error("登録クリップの件数が上限に達しています")]
    LimitReached,
    /// 登録画像のサイズまたはピクセル数が上限を超える
    #[error("登録画像のサイズが上限を超えています")]
    ImageTooLarge,
    /// 登録画像の形式または内容が不正
    #[error("登録画像の形式が不正です")]
    ImageInvalid,
}

impl AddRegisteredClipError {
    /// ユーザー向けのエラーメッセージを返す
    #[must_use]
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::Empty => "登録クリップが空です",
            Self::TooLarge => "登録クリップのサイズが上限を超えています",
            Self::LimitReached => "登録クリップの件数が上限に達しています",
            Self::ImageTooLarge => "登録画像のサイズが上限を超えています",
            Self::ImageInvalid => "登録画像の形式が不正です",
        }
    }
}
