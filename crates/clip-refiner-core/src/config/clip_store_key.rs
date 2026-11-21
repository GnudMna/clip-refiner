#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::paths::get_config_dir;
use super::permissions::restrict_private_file_permissions;
use crate::error::ConfigError;
use crate::native::dpapi;
use crate::security::{CIPHER_KEY_LEN, generate_cipher_key};

type Result<T> = std::result::Result<T, ConfigError>;

// ======================================================================
// 定数
// ======================================================================
const CLIP_STORE_KEY_FILE: &str = "registered-clips.key";
static CLIP_STORE_KEY: OnceLock<[u8; CIPHER_KEY_LEN]> = OnceLock::new();

#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
thread_local! {
    static THREAD_CLIP_STORE_KEY: RefCell<Option<[u8; CIPHER_KEY_LEN]>> = const { RefCell::new(None) };
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 登録クリップ暗号化鍵を取得する
///
/// 鍵ファイルが無い場合は新規生成して保存する
///
/// # Returns
/// * `Result<[u8; CIPHER_KEY_LEN]>` - 暗号化鍵。取得または生成に失敗した場合は `Err`
///
/// # Errors
/// 設定ディレクトリ取得、鍵ファイル読み書き、鍵生成、DPAPI のいずれかに失敗した場合 (パーミッション設定の失敗は警告のみ)
pub fn clip_store_key() -> Result<[u8; CIPHER_KEY_LEN]> {
    #[cfg(any(test, debug_assertions, feature = "test-helpers"))]
    if let Some(key) = thread_local_clip_store_key() {
        return Ok(key);
    }

    if let Some(key) = CLIP_STORE_KEY.get() {
        return Ok(*key);
    }

    let key = load_or_create_key()?;
    let _ = CLIP_STORE_KEY.set(key);
    Ok(key)
}

#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
fn thread_local_clip_store_key() -> Option<[u8; CIPHER_KEY_LEN]> {
    THREAD_CLIP_STORE_KEY.with(|cell| {
        if let Some(key) = *cell.borrow() {
            return Some(key);
        }
        let key = load_or_create_key().ok()?;
        *cell.borrow_mut() = Some(key);
        Some(key)
    })
}

/// 登録クリップ暗号化鍵を確実に存在させる
///
/// # Returns
/// * `Result<()>` - 鍵の取得または生成に成功した場合は `Ok(())`
///
/// # Errors
/// [`clip_store_key`] と同様
pub fn ensure_clip_store_key() -> Result<()> {
    clip_store_key().map(|_| ())
}

/// テスト用にキャッシュ済み暗号化鍵を破棄する
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub(crate) fn clear_clip_store_key_cache() {
    THREAD_CLIP_STORE_KEY.with(|cell| *cell.borrow_mut() = None);
}

// ======================================================================
// 鍵ファイル I/O
// ======================================================================
fn clip_store_key_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join(CLIP_STORE_KEY_FILE))
}

fn load_or_create_key() -> Result<[u8; CIPHER_KEY_LEN]> {
    let path = clip_store_key_path()?;
    if path.is_file() {
        return read_key_file(&path);
    }

    let key = generate_cipher_key()?;
    write_key_file(&path, &key)?;
    Ok(key)
}

fn read_key_file(path: &PathBuf) -> Result<[u8; CIPHER_KEY_LEN]> {
    let bytes = fs::read(path).map_err(ConfigError::Read)?;
    let plain = dpapi::unprotect(&bytes, CIPHER_KEY_LEN)?;
    let mut key = [0u8; CIPHER_KEY_LEN];
    key.copy_from_slice(&plain);
    Ok(key)
}

fn write_key_file(path: &PathBuf, key: &[u8; CIPHER_KEY_LEN]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::CreateDir)?;
    }

    let protected = dpapi::protect(key)?;
    fs::write(path, protected).map_err(ConfigError::Write)?;
    if let Err(e) = restrict_private_file_permissions(path) {
        tracing::warn!("登録クリップ暗号化鍵のパーミッション設定に失敗: {:?}", e);
    }
    Ok(())
}
