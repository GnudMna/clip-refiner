//! DPAPI によるデータ保護

use std::ptr;

use anyhow::{Context, Result, bail};

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

// ======================================================================
// パブリック関数
// ======================================================================
/// 平文データを DPAPI で保護する
pub(crate) fn protect(data: &[u8]) -> Result<Vec<u8>> {
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: blob_len_u32(data.len())?,
        pbData: data.as_ptr().cast_mut(),
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
        bail!("DPAPI によるデータ保護に失敗");
    }

    let protected =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(protected)
}

/// DPAPI 保護済みデータを平文へ復元する
pub(crate) fn unprotect(blob: &[u8], expected_len: usize) -> Result<Vec<u8>> {
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
        bail!("DPAPI によるデータ復号に失敗");
    }

    let plain = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    if plain.len() != expected_len {
        unsafe {
            LocalFree(output.pbData.cast());
        }
        bail!("DPAPI 復号データの長さが不正です");
    }

    let data = plain.to_vec();
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(data)
}

// ======================================================================
// プライベート関数
// ======================================================================
fn blob_len_u32(len: usize) -> Result<u32> {
    u32::try_from(len).context("DPAPI 入力データ長が大きすぎます")
}
