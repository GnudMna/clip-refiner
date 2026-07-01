#[cfg(any(test, debug_assertions))]
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::paths::get_config_dir;
use super::permissions::restrict_private_file_permissions;

use anyhow::{Context, Result};

// ======================================================================
// 定数
// ======================================================================
const CLIP_STORE_KEY_FILE: &str = "registered-clips.key";
const KEY_LEN: usize = 32;

static CLIP_STORE_KEY: OnceLock<[u8; KEY_LEN]> = OnceLock::new();

#[cfg(any(test, debug_assertions))]
thread_local! {
    static THREAD_CLIP_STORE_KEY: RefCell<Option<[u8; KEY_LEN]>> = const { RefCell::new(None) };
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 登録クリップ暗号化鍵を取得する
///
/// 鍵ファイルが無い場合は新規生成して保存する
pub fn clip_store_key() -> Result<[u8; KEY_LEN]> {
    #[cfg(any(test, debug_assertions))]
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

#[cfg(any(test, debug_assertions))]
fn thread_local_clip_store_key() -> Option<[u8; KEY_LEN]> {
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
pub fn ensure_clip_store_key() -> Result<()> {
    clip_store_key().map(|_| ())
}

/// テスト用にキャッシュ済み暗号化鍵を破棄する
#[cfg(any(test, debug_assertions))]
pub(crate) fn clear_clip_store_key_cache() {
    THREAD_CLIP_STORE_KEY.with(|cell| *cell.borrow_mut() = None);
}

// ======================================================================
// 鍵ファイル I/O
// ======================================================================
fn clip_store_key_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join(CLIP_STORE_KEY_FILE))
}

fn load_or_create_key() -> Result<[u8; KEY_LEN]> {
    let path = clip_store_key_path()?;
    if path.is_file() {
        return read_key_file(&path);
    }

    let key = generate_key()?;
    write_key_file(&path, &key)?;
    Ok(key)
}

fn generate_key() -> Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    getrandom::fill(&mut key).context("登録クリップ暗号化鍵の生成に失敗")?;
    Ok(key)
}

fn read_key_file(path: &PathBuf) -> Result<[u8; KEY_LEN]> {
    let bytes = fs::read(path)
        .with_context(|| format!("登録クリップ暗号化鍵の読み込みに失敗: {}", path.display()))?;
    platform::unprotect_key(&bytes)
}

fn write_key_file(path: &PathBuf, key: &[u8; KEY_LEN]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("設定ディレクトリの作成に失敗")?;
    }

    let protected = platform::protect_key(key)?;
    fs::write(path, protected)
        .with_context(|| format!("登録クリップ暗号化鍵の書き込みに失敗: {}", path.display()))?;
    restrict_private_file_permissions(path)?;
    Ok(())
}

// ======================================================================
// プラットフォーム別鍵保護
// ======================================================================
#[cfg(windows)]
mod platform {
    use std::ptr;

    use anyhow::{Context, Result, bail};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };

    use super::KEY_LEN;

    fn blob_len_u32(len: usize) -> Result<u32> {
        u32::try_from(len).context("DPAPI 入力データ長が大きすぎます")
    }

    /// 平文鍵を DPAPI で保護する
    pub(super) fn protect_key(key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: blob_len_u32(key.len())?,
            pbData: key.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        let ok = unsafe {
            CryptProtectData(
                &raw mut input,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &raw mut output,
            )
        };
        if ok == 0 {
            bail!("登録クリップ暗号化鍵の DPAPI 保護に失敗");
        }

        let protected =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe {
            LocalFree(output.pbData.cast());
        }
        Ok(protected)
    }

    /// DPAPI 保護済み鍵を平文へ復元する
    pub(super) fn unprotect_key(blob: &[u8]) -> Result<[u8; KEY_LEN]> {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: blob_len_u32(blob.len())?,
            pbData: blob.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        let ok = unsafe {
            CryptUnprotectData(
                &raw mut input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &raw mut output,
            )
        };
        if ok == 0 {
            bail!("登録クリップ暗号化鍵の DPAPI 復号に失敗");
        }

        let plain = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
        if plain.len() != KEY_LEN {
            unsafe {
                LocalFree(output.pbData.cast());
            }
            bail!("登録クリップ暗号化鍵の長さが不正です");
        }

        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(plain);
        unsafe {
            LocalFree(output.pbData.cast());
        }
        Ok(key)
    }
}

#[cfg(not(windows))]
mod platform {
    use anyhow::{Result, bail};

    use super::KEY_LEN;

    /// Unix では所有者限定パーミッション付きで平文鍵を保存する
    pub(super) fn protect_key(key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
        Ok(key.to_vec())
    }

    /// Unix では平文鍵ファイルをそのまま読み込む
    pub(super) fn unprotect_key(blob: &[u8]) -> Result<[u8; KEY_LEN]> {
        if blob.len() != KEY_LEN {
            bail!("登録クリップ暗号化鍵の長さが不正です");
        }
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(blob);
        Ok(key)
    }
}
