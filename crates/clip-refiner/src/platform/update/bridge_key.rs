use super::bridge_key_obfuscate::{OBFUSCATE_MASK_LEN, xor_obfuscate};

use crate::security::{SecretString, decrypt_bytes, secret_from, zeroize_cipher_key};

use zeroize::Zeroize;

include!(concat!(env!("OUT_DIR"), "/bridge_key_blob.rs"));

// ======================================================================
// パブリック関数
// ======================================================================
/// ビルド時に暗号化して埋め込んだ bridge ダウンロードキーを返す
///
/// 環境変数 `CLIP_REFINER_KEY` がビルド時に設定されていれば復号した値
/// 未設定・空・復号失敗のときは `None` (無認証 GET)
/// 平文は戻り値の破棄時にゼロ化する
///
/// # Returns
/// * `Option<SecretString>` - 埋め込みキー
pub(crate) fn bridge_download_key() -> Option<SecretString> {
    if !BRIDGE_KEY_PRESENT {
        return None;
    }
    unwrap_blob(BRIDGE_KEY_BLOB, &BRIDGE_KEY_WRAP, &BRIDGE_KEY_MASK)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// XOR を外して ChaCha20-Poly1305 で復号する
///
/// # Arguments
/// * `blob` - XOR 済みの `nonce || ciphertext`
/// * `wrap_obfuscated` - XOR 済みの wrap key
/// * `mask` - XOR マスク
///
/// # Returns
/// * `Option<SecretString>` - 成功時は平文キー
fn unwrap_blob(
    blob: &[u8],
    wrap_obfuscated: &[u8; OBFUSCATE_MASK_LEN],
    mask: &[u8; OBFUSCATE_MASK_LEN],
) -> Option<SecretString> {
    let mut wrap_key = *wrap_obfuscated;
    let mut blob = blob.to_vec();
    xor_obfuscate(&mut wrap_key, mask);
    xor_obfuscate(&mut blob, mask);

    let decrypted = decrypt_bytes(&wrap_key, &blob);
    zeroize_cipher_key(&mut wrap_key);
    blob.zeroize();

    let Ok(bytes) = decrypted else {
        crate::log_warn!("埋め込みダウンロードキーの復号に失敗");
        return None;
    };

    let text = match String::from_utf8(bytes) {
        Ok(text) if !text.is_empty() => text,
        Ok(_) => return None,
        Err(err) => {
            let mut invalid = err.into_bytes();
            invalid.zeroize();
            crate::log_warn!("埋め込みダウンロードキーが UTF-8 ではありません");
            return None;
        }
    };
    Some(secret_from(text))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::security::encrypt_bytes;

    /// 合成 blob を復号して平文を復元できること
    #[test]
    fn unwrap_blob_roundtrips_synthetic_key() {
        let plaintext = "synthetic-bridge-download-key";
        let wrap_key = [0x11u8; OBFUSCATE_MASK_LEN];
        let mask = [0x5au8; OBFUSCATE_MASK_LEN];
        let mut blob = encrypt_bytes(&wrap_key, plaintext.as_bytes()).expect("encrypt");
        let mut wrap_stored = wrap_key;
        xor_obfuscate(&mut blob, &mask);
        xor_obfuscate(&mut wrap_stored, &mask);

        assert!(
            !blob
                .windows(plaintext.len())
                .any(|window| window == plaintext.as_bytes()),
            "XOR 後の blob に平文が残っている"
        );

        let got = unwrap_blob(&blob, &wrap_stored, &mask).expect("unwrap");
        assert_eq!(got.as_str(), plaintext);
    }

    /// wrap key が壊れている場合は None を返すこと
    #[test]
    fn unwrap_blob_returns_none_on_wrong_wrap_key() {
        let plaintext = "synthetic-bridge-download-key";
        let wrap_key = [0x11u8; OBFUSCATE_MASK_LEN];
        let mask = [0x5au8; OBFUSCATE_MASK_LEN];
        let mut blob = encrypt_bytes(&wrap_key, plaintext.as_bytes()).expect("encrypt");
        xor_obfuscate(&mut blob, &mask);
        let wrap_stored = [0x22u8; OBFUSCATE_MASK_LEN];

        assert!(unwrap_blob(&blob, &wrap_stored, &mask).is_none());
    }

    /// 埋め込みキーがあるときは復号でき、blob に平文を含まないこと
    #[test]
    fn embedded_bridge_key_decrypts_without_plaintext_in_blob() {
        let key = bridge_download_key();
        assert_eq!(key.is_some(), BRIDGE_KEY_PRESENT);
        let Some(key) = key else {
            return;
        };
        assert!(!key.is_empty());
        let plain = key.as_bytes();
        if plain.len() >= 8 {
            assert!(
                !BRIDGE_KEY_BLOB
                    .windows(plain.len())
                    .any(|window| window == plain),
                "埋め込み blob に平文キーが含まれている"
            );
        }
    }
}
