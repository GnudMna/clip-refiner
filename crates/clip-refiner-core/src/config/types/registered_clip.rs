use std::fmt;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::security::SecretString;

// ======================================================================
// 登録クリップ
// ======================================================================
/// クリップボードへコピーするための登録クリップ (テキストまたは画像)
///
/// 本文は `text_ciphertext` として AEAD 暗号文のみを保持し、平文は常駐させない
/// `registered-clips.dat` に暗号化して保存される
/// `image_file` が指定されている場合は画像登録として扱う
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredClip {
    /// 一覧表示用のラベル
    pub label: String,
    /// 表示用プレビュー (登録時に `format_public_snippet` で生成。平文本文ではない)
    #[serde(default)]
    pub preview: String,
    /// 本文の AEAD 暗号文 (`nonce || ciphertext`)。画像登録時は空
    #[serde(default, with = "text_ciphertext_serde")]
    pub text_ciphertext: Vec<u8>,
    /// 登録画像ファイル名 (`registered-images` ディレクトリ内の相対パス)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_file: Option<String>,
    /// コピー時に `{date}` などのプレースホルダを展開するか。画像登録では常に `false`
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub template: bool,
}

impl fmt::Debug for RegisteredClip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredClip")
            .field("label", &self.label)
            .field("preview", &self.preview)
            .field(
                "text_ciphertext",
                &if self.text_ciphertext.is_empty() {
                    "[]"
                } else {
                    "..."
                },
            )
            .field("image_file", &self.image_file)
            .field("template", &self.template)
            .finish()
    }
}

impl RegisteredClip {
    /// 画像登録かどうかを返す
    ///
    /// # Arguments
    /// * `self` - 判定対象の登録クリップ
    ///
    /// # Returns
    /// * `bool` - `image_file` が設定されている場合は `true`
    pub fn is_image(&self) -> bool {
        self.image_file.is_some()
    }
}

// ======================================================================
// text_ciphertext の base64 シリアライズ
// ======================================================================
mod text_ciphertext_serde {
    use super::*;

    /// バイト列を base64 文字列としてシリアライズする
    ///
    /// # Arguments
    /// * `bytes` - バイト列
    /// * `serializer` - `Serializer`
    ///
    /// # Returns
    /// * `Result<S::Ok, S::Error>` - 成功時は `Ok`、失敗時は `Err`
    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if bytes.is_empty() {
            serializer.serialize_str("")
        } else {
            serializer.serialize_str(&STANDARD.encode(bytes))
        }
    }

    /// base64 文字列からバイト列へデシリアライズする
    ///
    /// # Arguments
    /// * `deserializer` - `Deserializer`
    ///
    /// # Returns
    /// * `Result<Vec<u8>, D::Error>` - 成功時は `Ok`、失敗時は `Err`
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        if encoded.is_empty() {
            return Ok(Vec::new());
        }
        STANDARD
            .decode(encoded.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

// ======================================================================
// 登録クリップボード内容
// ======================================================================
/// 登録済みクリップボード内容 (コピー用)
#[derive(Clone)]
pub enum ResolvedClip {
    /// テキスト (破棄時にゼロ化)
    Text(SecretString),
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

impl fmt::Debug for ResolvedClip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(_) => f.debug_tuple("Text").field(&"...").finish(),
            Self::Image {
                width,
                height,
                rgba,
            } => f
                .debug_struct("Image")
                .field("width", width)
                .field("height", height)
                .field("rgba_len", &rgba.len())
                .finish(),
        }
    }
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
    /// 本文の暗号化に失敗
    #[error("登録クリップの暗号化に失敗しました")]
    EncryptFailed,
}

impl AddRegisteredClipError {
    /// ユーザー向けのエラーメッセージを返す
    ///
    /// # Arguments
    /// * `self` - 追加失敗理由
    ///
    /// # Returns
    /// * `&'static str` - UI 表示用の固定メッセージ
    #[must_use]
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::Empty => "登録クリップが空です",
            Self::TooLarge => "登録クリップのサイズが上限を超えています",
            Self::LimitReached => "登録クリップの件数が上限に達しています",
            Self::ImageTooLarge => "登録画像のサイズが上限を超えています",
            Self::ImageInvalid => "登録画像の形式が不正です",
            Self::EncryptFailed => "登録クリップの暗号化に失敗しました",
        }
    }
}
