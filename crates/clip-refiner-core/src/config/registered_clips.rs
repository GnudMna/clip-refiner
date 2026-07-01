use std::fs;
use std::path::{Path, PathBuf};

use super::clip_store_key::{clip_store_key, ensure_clip_store_key};
use super::paths::get_config_dir;
use super::permissions::restrict_private_file_permissions;
use super::registered_images::migrate_plain_image_to_encrypted;
use super::types::RegisteredClip;

use crate::consts;
use crate::security::{decrypt_bytes, encrypt_bytes};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

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

// ======================================================================
// パブリック関数
// ======================================================================
/// 暗号化済み登録クリップファイルを読み込む
///
/// ファイルが無い場合は空配列を返す
///
/// # Returns
/// * `Result<Vec<RegisteredClip>>` - 復号・解析済みの登録クリップ一覧
pub fn load_registered_clips() -> Result<Vec<RegisteredClip>> {
    ensure_clip_store_key()?;

    let path = registered_clips_path()?;
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let bytes = fs::read(&path)
        .with_context(|| format!("登録クリップファイルの読み込みに失敗: {}", path.display()))?;
    if bytes.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        bail!("登録クリップファイルのサイズが上限を超えています");
    }

    let key = clip_store_key()?;
    let json = decrypt_bytes(&key, &bytes).context("登録クリップファイルの復号に失敗")?;
    let payload: RegisteredClipsPayload =
        serde_json::from_slice(&json).context("登録クリップファイルの JSON 解析に失敗")?;
    Ok(payload.clips)
}

/// 登録クリップ一覧を暗号化ファイルへ保存する
///
/// 空の場合はファイルを削除する
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
    .context("登録クリップの JSON シリアライズに失敗")?;
    if json.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        bail!("登録クリップのシリアライズサイズが上限を超えています");
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &json)?;
    if encrypted.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        bail!("登録クリップファイルの暗号化サイズが上限を超えています");
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("設定ディレクトリの作成に失敗")?;
    }
    fs::write(&path, encrypted)
        .with_context(|| format!("登録クリップファイルの書き込みに失敗: {}", path.display()))?;
    restrict_private_file_permissions(&path)?;
    Ok(())
}

/// レガシー `.png` 登録画像を暗号化 `.enc` へ移行する
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 暗号化ファイルの往復で平文 JSON が残らないこと
    #[test]
    fn save_and_load_do_not_persist_plaintext() {
        crate::test_helpers::with_temp_config_dir(|| {
            let clips = vec![RegisteredClip {
                label: "secret-label".into(),
                text: "secret-body".into(),
                image_file: None,
            }];

            save_registered_clips(&clips).expect("save");
            let loaded = load_registered_clips().expect("load");
            assert_eq!(loaded, clips);

            let path = registered_clips_path().expect("path");
            let bytes = fs::read(path).expect("read");
            let content = String::from_utf8_lossy(&bytes);
            assert!(!content.contains("secret-body"));
            assert!(!content.contains("secret-label"));
        });
    }
}
