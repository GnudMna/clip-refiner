use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use zeroize::Zeroize;

// ======================================================================
// 定数
// ======================================================================
/// ChaCha20-Poly1305 暗号化鍵のバイト長
pub const CIPHER_KEY_LEN: usize = 32;

const NONCE_LEN: usize = 12;

// ======================================================================
// 鍵の生成と破棄
// ======================================================================
/// ChaCha20-Poly1305 用のランダム鍵を生成する
///
/// # Returns
/// * `Result<[u8; CIPHER_KEY_LEN]>` - 生成した暗号化鍵
///
/// # Errors
/// OS の乱数取得に失敗した場合
pub fn generate_cipher_key() -> Result<[u8; CIPHER_KEY_LEN]> {
    let mut key = [0u8; CIPHER_KEY_LEN];
    getrandom::fill(&mut key).context("暗号化鍵の生成に失敗")?;
    Ok(key)
}

/// 暗号化鍵をメモリ上からゼロ化する
///
/// # Arguments
/// * `key` - ゼロ化対象の暗号化鍵
pub fn zeroize_cipher_key(key: &mut [u8; CIPHER_KEY_LEN]) {
    key.zeroize();
}

// ======================================================================
// バイナリ暗号化
// ======================================================================
/// 平文バイト列を ChaCha20-Poly1305 で暗号化する
///
/// 戻り値は `nonce (12 バイト) || ciphertext` 形式
///
/// # Arguments
/// * `key` - 暗号化鍵
/// * `plaintext` - 暗号化対象の平文
///
/// # Returns
/// * `Result<Vec<u8>>` - ノンス付き暗号文
///
/// # Errors
/// ノンス生成または暗号化に失敗した場合
pub fn encrypt_bytes(key: &[u8; CIPHER_KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>> {
    let key = key_array(key)?;
    let cipher = ChaCha20Poly1305::new(&key);
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).context("暗号化ノンスの生成に失敗")?;
    let nonce_arr = nonce_array(&nonce)?;
    let ciphertext = cipher
        .encrypt(&nonce_arr, plaintext)
        .map_err(|e| anyhow::anyhow!("暗号化に失敗: {e:?}"))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// `encrypt_bytes` で生成したバイト列を復号する
///
/// # Arguments
/// * `key` - 暗号化鍵
/// * `blob` - ノンス付き暗号文
///
/// # Returns
/// * `Result<Vec<u8>>` - 復号した平文
///
/// # Errors
/// 暗号文が短すぎる、または復号に失敗した場合
pub fn decrypt_bytes(key: &[u8; CIPHER_KEY_LEN], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() <= NONCE_LEN {
        bail!("暗号文が短すぎます");
    }

    let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
    let key = key_array(key)?;
    let cipher = ChaCha20Poly1305::new(&key);
    let nonce = nonce_array(nonce)?;
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("復号に失敗: {e:?}"))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// バイト列を `Key` に変換する
///
/// # Arguments
/// * `key` - 暗号化鍵
///
/// # Returns
/// * `Result<Key>` - ChaCha20-Poly1305 用の鍵
///
/// # Errors
/// 鍵長が一致しない場合 (呼び出し側の型で保証されるため通常は起きない)
fn key_array(key: &[u8; CIPHER_KEY_LEN]) -> Result<Key> {
    Key::try_from(key.as_slice()).map_err(|_| anyhow::anyhow!("不正な鍵長"))
}

/// バイト列を `Nonce` に変換する
///
/// # Arguments
/// * `nonce` - ノンス (12 バイト)
///
/// # Returns
/// * `Result<Nonce>` - ChaCha20-Poly1305 用のノンス
///
/// # Errors
/// ノンス長が 12 バイトでない場合
fn nonce_array(nonce: &[u8]) -> Result<Nonce> {
    Nonce::try_from(nonce).map_err(|_| anyhow::anyhow!("不正なノンス長"))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; CIPHER_KEY_LEN] = [7; CIPHER_KEY_LEN];

    /// バイト列の暗号化往復が成功すること
    #[test]
    fn encrypt_bytes_roundtrip() {
        let plaintext = b"secret clip text";
        let encrypted = encrypt_bytes(&TEST_KEY, plaintext).expect("encrypt");
        assert_ne!(encrypted.as_slice(), plaintext);
        let decrypted = decrypt_bytes(&TEST_KEY, &encrypted).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }
}
