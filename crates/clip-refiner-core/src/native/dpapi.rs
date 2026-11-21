//! DPAPI によるデータ保護

use std::ptr;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;
use zeroize::{Zeroize, Zeroizing};

use crate::error::ConfigError;

type Result<T> = std::result::Result<T, ConfigError>;

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

    // SAFETY: `input` は呼び出し中に生存する `data` を指し、DPAPI は入力を書き換えない
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
    .map_err(|e| ConfigError::dpapi(format!("DPAPI によるデータ保護に失敗: {e}")))?;

    // SAFETY: 成功時の `output` は DPAPI が確保した `cbData` バイトの有効な領域を指す
    let protected =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: `output.pbData` は DPAPI が `LocalAlloc` で確保した領域で、コピー後は参照しない
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    Ok(protected)
}

/// DPAPI 保護済みデータを平文へ復元する
///
/// DPAPI が確保した平文領域は解放前にゼロ化し、戻り値も破棄時にゼロ化する
/// 復号結果の長さが `expected_len` と異なる場合はエラーにする
pub(crate) fn unprotect(blob: &[u8], expected_len: usize) -> Result<Zeroizing<Vec<u8>>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: blob_len_u32(blob.len())?,
        pbData: blob.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: `input` は呼び出し中に生存する `blob` を指し、DPAPI は入力を書き換えない
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
    .map_err(|e| ConfigError::dpapi(format!("DPAPI によるデータ復号に失敗: {e}")))?;

    if output.pbData.is_null() {
        return Err(ConfigError::dpapi("DPAPI 復号データが空です"));
    }

    // SAFETY: 成功時の `output` は DPAPI が確保した `cbData` バイトの書き込み可能な領域を指す
    // 他から参照されない領域で、`plain` は `LocalFree` より前にのみ使う
    let plain = unsafe { std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize) };
    let data = (plain.len() == expected_len).then(|| Zeroizing::new(plain.to_vec()));
    plain.zeroize();
    // SAFETY: `output.pbData` は DPAPI が確保した領域で、ゼロ化後は `plain` を参照しない
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    data.ok_or_else(|| ConfigError::dpapi("DPAPI 復号データの長さが不正です"))
}

// ======================================================================
// プライベート関数
// ======================================================================
fn blob_len_u32(len: usize) -> Result<u32> {
    u32::try_from(len).map_err(|_| ConfigError::dpapi("DPAPI 入力データ長が大きすぎます"))
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
