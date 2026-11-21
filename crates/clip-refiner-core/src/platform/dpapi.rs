//! DPAPI によるデータ保護

use std::ptr;

use anyhow::{Context, Result, bail};

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

// ======================================================================
// パブリック関数
// ======================================================================
/// 平文データを DPAPI で保護する
pub(crate) fn protect(data: &[u8]) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: blob_len_u32(data.len())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        CryptProtectData(
            &raw const input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }
    .context("DPAPI によるデータ保護に失敗")?;

    let protected =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    Ok(protected)
}

/// DPAPI 保護済みデータを平文へ復元する
pub(crate) fn unprotect(blob: &[u8], expected_len: usize) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: blob_len_u32(blob.len())?,
        pbData: blob.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        CryptUnprotectData(
            &raw const input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }
    .context("DPAPI によるデータ復号に失敗")?;

    let plain = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    if plain.len() != expected_len {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        }
        bail!("DPAPI 復号データの長さが不正です");
    }

    let data = plain.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    Ok(data)
}

// ======================================================================
// プライベート関数
// ======================================================================
fn blob_len_u32(len: usize) -> Result<u32> {
    u32::try_from(len).context("DPAPI 入力データ長が大きすぎます")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// DPAPI の protect / unprotect が往復できるようにすること
    #[test]
    fn dpapi_round_trip_preserves_plaintext() {
        let plain = b"clip-refiner dpapi round-trip";
        let protected = protect(plain).expect("protect");
        assert_ne!(protected.as_slice(), plain);

        let restored = unprotect(&protected, plain.len()).expect("unprotect");
        assert_eq!(restored.as_slice(), plain);
    }

    /// 復号時の期待長と不一致なら失敗すること
    #[test]
    fn dpapi_unprotect_rejects_length_mismatch() {
        let plain = b"length-check";
        let protected = protect(plain).expect("protect");

        let err = unprotect(&protected, plain.len() + 1).unwrap_err();
        assert!(err.to_string().contains("長さが不正"));
    }
}
