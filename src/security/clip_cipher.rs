use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

// ======================================================================
// 定数
// ======================================================================
const NONCE_LEN: usize = 12;

// ======================================================================
// バイナリ暗号化
// ======================================================================
/// 平文バイト列を ChaCha20-Poly1305 で暗号化する
///
/// 戻り値は `nonce (12 バイト) || ciphertext` 形式
pub fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).context("登録クリップ暗号化ノンスの生成に失敗")?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|e| anyhow::anyhow!("登録クリップの暗号化に失敗: {e:?}"))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// `encrypt_bytes` で生成したバイト列を復号する
pub fn decrypt_bytes(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() <= NONCE_LEN {
        bail!("登録クリップの暗号文が短すぎます");
    }

    let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| anyhow::anyhow!("登録クリップの復号に失敗: {e:?}"))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [7; 32];

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
