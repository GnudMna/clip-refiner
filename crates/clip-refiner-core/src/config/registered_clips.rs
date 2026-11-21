use std::fs;
use std::path::{Path, PathBuf};

use super::atomic_io::write_bytes_atomically;
use super::clip_store_key::{clip_store_key, ensure_clip_store_key};
use super::paths::get_config_dir;
use super::permissions::restrict_private_file_permissions;
use super::registered_images::migrate_plain_image_to_encrypted;
use super::types::RegisteredClip;
use crate::consts;
use crate::error::ConfigError;
use crate::security::{
    SecretString, decrypt_bytes, encrypt_bytes, format_public_snippet, is_within_clipboard_limit,
    secret_from,
};

use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, ConfigError>;

fn clips_err(msg: impl Into<String>) -> ConfigError {
    ConfigError::RegisteredClipsSave(msg.into())
}

// ======================================================================
// 定数
// ======================================================================
const REGISTERED_CLIPS_FILE: &str = "registered-clips.dat";

// ======================================================================
// ファイルペイロード
// ======================================================================
#[derive(Debug, Serialize, Deserialize)]
struct RegisteredClipsPayload {
    clips: Vec<RegisteredClip>,
}

/// 外側 blob 内のワイヤ形式 (新形式 / 平文 `text` レガシーを両対応)
#[derive(Debug, Deserialize)]
struct RegisteredClipWire {
    #[serde(default)]
    label: String,
    #[serde(default)]
    preview: String,
    /// レガシー: 平文本文
    #[serde(default)]
    text: Option<String>,
    /// 新形式: base64 暗号文
    #[serde(default)]
    text_ciphertext: Option<String>,
    #[serde(default)]
    image_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisteredClipsPayloadWire {
    clips: Vec<RegisteredClipWire>,
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 暗号化済み登録クリップファイルを読み込む
///
/// ファイルが無い場合は空配列を返す
/// 平文 `text` フィールドを持つ旧形式は暗号文へ移行する
///
/// # Returns
/// * `Result<(Vec<RegisteredClip>, bool)>` - 登録クリップ一覧と、形式移行が発生したか
///
/// # Errors
/// 鍵取得、ファイル読み込み、復号、JSON 解析、本文暗号化のいずれかに失敗した場合
pub fn load_registered_clips() -> Result<(Vec<RegisteredClip>, bool)> {
    ensure_clip_store_key()?;

    let path = registered_clips_path()?;
    if !path.is_file() {
        return Ok((Vec::new(), false));
    }

    let bytes = fs::read(&path).map_err(|e| {
        clips_err(format!(
            "登録クリップファイルの読み込みに失敗: {}: {e}",
            path.display()
        ))
    })?;
    if bytes.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        return Err(clips_err(
            "登録クリップファイルのサイズが上限を超えています",
        ));
    }

    let key = clip_store_key()?;
    let json = decrypt_bytes(&key, &bytes)
        .map_err(|e| clips_err(format!("登録クリップファイルの復号に失敗: {e}")))?;
    let payload: RegisteredClipsPayloadWire = serde_json::from_slice(&json)
        .map_err(|e| clips_err(format!("登録クリップファイルの JSON 解析に失敗: {e}")))?;

    let mut migrated = false;
    let mut clips = Vec::with_capacity(payload.clips.len());
    for wire in payload.clips {
        match registered_clip_from_wire(wire) {
            Ok((clip, entry_migrated)) => {
                migrated |= entry_migrated;
                clips.push(clip);
            }
            Err(err) => {
                tracing::warn!("登録クリップの読み込みをスキップ: {:?}", err);
                migrated = true;
            }
        }
    }
    Ok((clips, migrated))
}

/// 登録クリップ一覧を暗号化ファイルへ保存する
///
/// 空の場合はファイルを削除する
/// 平文本文はシリアライズしない (暗号文フィールドのみ)
///
/// # Arguments
/// * `clips` - 保存する登録クリップ一覧
///
/// # Returns
/// * `Result<()>` - 保存または削除に成功した場合は `Ok(())`
///
/// # Errors
/// 鍵取得、シリアライズ、暗号化、ファイル書き込みのいずれかに失敗した場合
pub fn save_registered_clips(clips: &[RegisteredClip]) -> Result<()> {
    ensure_clip_store_key()?;

    let path = registered_clips_path()?;
    if clips.is_empty() {
        if path.is_file() {
            let _ = fs::remove_file(path);
        }
        return Ok(());
    }

    let json = serde_json::to_vec(&RegisteredClipsPayload {
        clips: clips.to_vec(),
    })
    .map_err(|e| clips_err(format!("登録クリップの JSON シリアライズに失敗: {e}")))?;
    if json.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        return Err(clips_err(
            "登録クリップのシリアライズサイズが上限を超えています",
        ));
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &json)?;
    if encrypted.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        return Err(clips_err(
            "登録クリップファイルの暗号化サイズが上限を超えています",
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::CreateDir)?;
    }
    write_bytes_atomically(&path, &encrypted).map_err(|e| {
        clips_err(format!(
            "登録クリップファイルの書き込みに失敗: {}: {e}",
            path.display()
        ))
    })?;
    if let Err(e) = restrict_private_file_permissions(&path) {
        tracing::warn!("登録クリップファイルのパーミッション設定に失敗: {:?}", e);
    }
    Ok(())
}

/// レガシー `.png` 登録画像を暗号化 `.enc` へ移行する
///
/// # Arguments
/// * `clips` - 移行対象の登録クリップ一覧
///
/// # Returns
/// * `bool` - 1 件以上移行した場合は `true`
pub fn migrate_legacy_clip_images(clips: &mut [RegisteredClip]) -> bool {
    let mut migrated = false;
    for clip in clips {
        let Some(ref image_file) = clip.image_file else {
            continue;
        };
        if is_encrypted_image_filename(image_file) {
            continue;
        }
        match migrate_plain_image_to_encrypted(image_file) {
            Ok(new_file) => {
                clip.image_file = Some(new_file);
                migrated = true;
            }
            Err(err) => {
                tracing::warn!("登録画像の暗号化移行に失敗 ({}): {:?}", image_file, err);
                clip.image_file = None;
                migrated = true;
            }
        }
    }
    migrated
}

/// 平文本文を AEAD 暗号文と表示用プレビューへ変換する
///
/// # Arguments
/// * `text` - 暗号化する平文
///
/// # Returns
/// * `Result<(Vec<u8>, String)>` - `(text_ciphertext, preview)`
///
/// # Errors
/// 鍵取得または暗号化に失敗した場合
pub(crate) fn encrypt_registered_clip_text(text: &str) -> Result<(Vec<u8>, String)> {
    ensure_clip_store_key()?;
    let key = clip_store_key()?;
    let ciphertext = encrypt_bytes(&key, text.as_bytes())?;
    let preview = format_public_snippet(text, consts::REGISTERED_CLIP_PREVIEW_MAX_CHARS);
    Ok((ciphertext, preview))
}

/// 登録クリップ本文の暗号文を復号する
///
/// # Arguments
/// * `ciphertext` - `encrypt_bytes` 形式の暗号文
///
/// # Returns
/// * `Result<SecretString>` - 復号した平文 (破棄時にゼロ化)
///
/// # Errors
/// 鍵取得、復号、UTF-8 解釈のいずれかに失敗した場合
pub(crate) fn decrypt_registered_clip_text(ciphertext: &[u8]) -> Result<SecretString> {
    let key = clip_store_key()?;
    let plain = decrypt_bytes(&key, ciphertext)
        .map_err(|e| clips_err(format!("登録クリップ本文の復号に失敗: {e}")))?;
    let text = String::from_utf8(plain)
        .map_err(|e| clips_err(format!("登録クリップ本文が UTF-8 ではありません: {e}")))?;
    Ok(secret_from(text))
}

/// 平文ラベル・本文からメモリ用の登録クリップを組み立てる
///
/// # Arguments
/// * `label` - 表示ラベル (空なら正規化時に preview から補完)
/// * `text` - 平文本文
///
/// # Returns
/// * `Result<RegisteredClip>` - 暗号文を保持する登録クリップ
///
/// # Errors
/// 本文が空 / サイズ超過、または暗号化に失敗した場合
pub(crate) fn registered_clip_from_plain_text(label: String, text: &str) -> Result<RegisteredClip> {
    if text.trim().is_empty() {
        return Err(clips_err("登録クリップが空です"));
    }
    if !is_within_clipboard_limit(text) {
        return Err(clips_err("登録クリップのサイズが上限を超えています"));
    }
    let (text_ciphertext, preview) = encrypt_registered_clip_text(text)?;
    Ok(RegisteredClip {
        label,
        preview,
        text_ciphertext,
        image_file: None,
    })
}

// ======================================================================
// プライベート関数
// ======================================================================
fn registered_clips_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join(REGISTERED_CLIPS_FILE))
}

fn is_encrypted_image_filename(filename: &str) -> bool {
    Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("enc"))
}

/// ワイヤ形式をランタイム用 `RegisteredClip` へ変換する
fn registered_clip_from_wire(wire: RegisteredClipWire) -> Result<(RegisteredClip, bool)> {
    if wire.image_file.is_some() {
        return Ok((
            RegisteredClip {
                label: wire.label,
                preview: wire.preview,
                text_ciphertext: Vec::new(),
                image_file: wire.image_file,
            },
            false,
        ));
    }

    if let Some(encoded) = wire.text_ciphertext.filter(|s| !s.is_empty()) {
        let text_ciphertext = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            encoded.as_bytes(),
        )
        .map_err(|e| clips_err(format!("登録クリップ本文暗号文の base64 復号に失敗: {e}")))?;

        let mut clip = RegisteredClip {
            label: wire.label,
            preview: wire.preview,
            text_ciphertext,
            image_file: None,
        };
        let mut migrated = false;
        if clip.preview.is_empty()
            && let Ok(text) = decrypt_registered_clip_text(&clip.text_ciphertext)
        {
            clip.preview = format_public_snippet(&text, consts::REGISTERED_CLIP_PREVIEW_MAX_CHARS);
            migrated = true;
        }
        return Ok((clip, migrated));
    }

    if let Some(text) = wire.text {
        let clip = registered_clip_from_plain_text(wire.label, &text)?;
        return Ok((clip, true));
    }

    Err(clips_err("登録クリップに本文がありません"))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::encrypt_bytes;

    /// 暗号化ファイルの往復で平文 JSON がディスクに残らないようにすること
    #[test]
    fn save_and_load_do_not_persist_plaintext() {
        crate::test_helpers::with_temp_config_dir(|| {
            let clip =
                registered_clip_from_plain_text("secret-label".into(), "secret-body").expect("enc");
            let clips = vec![clip.clone()];

            save_registered_clips(&clips).expect("save");
            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(!migrated);
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].label, "secret-label");
            assert!(!loaded[0].text_ciphertext.is_empty());
            let plain = decrypt_registered_clip_text(&loaded[0].text_ciphertext).expect("dec");
            assert_eq!(plain.as_str(), "secret-body");

            let path = registered_clips_path().expect("path");
            let bytes = fs::read(path).expect("read");
            let content = String::from_utf8_lossy(&bytes);
            assert!(!content.contains("secret-body"));
            assert!(!content.contains("secret-label"));
        });
    }

    /// 平文 `text` を持つ旧形式を暗号文形式へ移行すること
    #[test]
    fn load_migrates_legacy_plaintext_text_field() {
        crate::test_helpers::with_temp_config_dir(|| {
            ensure_clip_store_key().expect("key");
            let key = clip_store_key().expect("key");
            let legacy_json = br#"{"clips":[{"label":"legacy","text":"plain-secret"}]}"#;
            let encrypted = encrypt_bytes(&key, legacy_json).expect("outer enc");
            let path = registered_clips_path().expect("path");
            fs::write(&path, encrypted).expect("write");

            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(migrated);
            assert_eq!(loaded.len(), 1);
            assert!(!loaded[0].text_ciphertext.is_empty());
            let plain = decrypt_registered_clip_text(&loaded[0].text_ciphertext).expect("dec");
            assert_eq!(plain.as_str(), "plain-secret");
            assert!(!loaded[0].preview.is_empty());

            // 再保存後の内側 JSON に平文本文キーが無いこと
            save_registered_clips(&loaded).expect("resave");
            let bytes = fs::read(path).expect("read");
            let inner = decrypt_bytes(&key, &bytes).expect("outer dec");
            let inner_str = String::from_utf8(inner).expect("utf8");
            assert!(!inner_str.contains("\"text\":"));
            assert!(inner_str.contains("text_ciphertext"));
            // preview には表示用断片が残るが、平文本文フィールドは無い
            assert!(inner_str.contains("\"preview\""));
        });
    }

    /// `Debug` が暗号文バイト列を展開しないようにすること
    #[test]
    fn registered_clip_debug_masks_ciphertext() {
        crate::test_helpers::with_temp_config_dir(|| {
            let clip = registered_clip_from_plain_text("lbl".into(), "secret-body").expect("enc");
            let debug = format!("{clip:?}");
            assert!(debug.contains("text_ciphertext: \"...\""));
            assert!(!debug.contains(&format!("{:?}", clip.text_ciphertext)));
        });
    }
}
