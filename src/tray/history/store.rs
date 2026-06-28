//! セッション限定の暗号化クリップボード履歴をメモリ上で保持する
//!
//! 起動時に生成した鍵で履歴を暗号化し、プロセス終了時に鍵とデータを破棄する

use crate::security::SecretString;

use anyhow::{Context, Result};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use zeroize::Zeroize;

// ======================================================================
// 暗号化エントリ
// ======================================================================
/// メモリ上に保持する暗号化済み履歴エントリ
struct EncryptedEntry {
    /// 重複判定用のコンテンツハッシュ (平文は保持しない)
    content_hash: [u8; 32],
    /// 暗号化に使用したノンス
    nonce: [u8; 12],
    /// 暗号化された本文
    ciphertext: Vec<u8>,
}

// ======================================================================
// 履歴ストア
// ======================================================================
/// セッション限定の暗号化クリップボード履歴ストア
///
/// 起動時に生成した鍵でメモリ内の履歴を暗号化して保持する
/// プロセス終了とともに鍵・履歴は破棄される (ディスクへは書き込まない)
pub struct EncryptedHistoryStore {
    key: [u8; 32],
    entries: Vec<EncryptedEntry>,
}

// ======================================================================
// 初期化
// ======================================================================
impl EncryptedHistoryStore {
    /// ランダム鍵で空のストアを生成する
    ///
    /// # Returns
    /// * `Result<Self>` - 空の暗号化履歴ストア。鍵生成に失敗した場合は `Err`
    pub fn new() -> Result<Self> {
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).context("履歴暗号化鍵の生成に失敗")?;
        Ok(Self {
            key,
            entries: Vec::new(),
        })
    }
}

// ======================================================================
// 参照
// ======================================================================
impl EncryptedHistoryStore {
    /// 保持している履歴件数を返す
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 指定インデックスの履歴を復号して返す
    ///
    /// # Arguments
    /// * `index` - 履歴ストア内のインデックス (0 が最新)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn entry_at(&self, index: usize) -> Option<SecretString> {
        let entry = self.entries.get(index)?;
        self.decrypt_entry(entry).ok()
    }
}

// ======================================================================
// 更新
// ======================================================================
impl EncryptedHistoryStore {
    /// 履歴を追加する
    ///
    /// 空文字は無視し、同一内容 (ハッシュ一致) は先頭へ移動する
    ///
    /// # Arguments
    /// * `text` - 追加する履歴テキスト
    /// * `limit` - 履歴の最大保持数
    pub fn add(&mut self, text: &str, limit: usize) {
        if text.trim().is_empty() {
            return;
        }

        let content_hash = Self::content_hash(text);

        if let Some(pos) = self
            .entries
            .iter()
            .position(|entry| entry.content_hash == content_hash)
        {
            self.entries.remove(pos);
        }

        let Some((nonce, ciphertext)) = self.encrypt(text) else {
            return;
        };

        self.entries.insert(
            0,
            EncryptedEntry {
                content_hash,
                nonce,
                ciphertext,
            },
        );

        if self.entries.len() > limit {
            self.entries.truncate(limit);
        }
    }

    /// 履歴をすべて削除する
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ======================================================================
// 暗号化ヘルパー
// ======================================================================
impl EncryptedHistoryStore {
    /// コンテンツの重複判定用ハッシュを計算する
    ///
    /// # Arguments
    /// * `text` - ハッシュ対象のテキスト
    ///
    /// # Returns
    /// * `[u8; 32]` - BLAKE3 によるコンテンツハッシュ
    fn content_hash(text: &str) -> [u8; 32] {
        *blake3::hash(text.as_bytes()).as_bytes()
    }

    /// 平文を暗号化する
    ///
    /// # Arguments
    /// * `text` - 暗号化対象のテキスト
    ///
    /// # Returns
    /// * `Option<([u8; 12], Vec<u8>)>` - 成功時は `(nonce, ciphertext)`、失敗時は `None`
    fn encrypt(&self, text: &str) -> Option<([u8; 12], Vec<u8>)> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).ok()?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), text.as_bytes())
            .inspect_err(|&e| {
                crate::log_error!("履歴の暗号化に失敗: {:?}", e);
            })
            .ok()?;
        Some((nonce, ciphertext))
    }

    /// 暗号化エントリを復号する
    ///
    /// # Arguments
    /// * `entry` - 復号対象の暗号化エントリ
    ///
    /// # Returns
    /// * `Result<SecretString, String>` - 成功時は復号済みテキスト、失敗時はエラーメッセージ
    fn decrypt_entry(&self, entry: &EncryptedEntry) -> Result<SecretString, String> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&entry.nonce), entry.ciphertext.as_ref())
            .map_err(|e| format!("履歴の復号に失敗: {e:?}"))?;
        String::from_utf8(plaintext)
            .map(SecretString::from)
            .map_err(|e| format!("履歴の UTF-8 変換に失敗: {e:?}"))
    }
}

// ======================================================================
// Drop
// ======================================================================
impl Drop for EncryptedHistoryStore {
    fn drop(&mut self) {
        self.key.zeroize();
        self.entries.clear();
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 暗号化ストアに平文バイト列がそのまま残らないこと
    #[test]
    fn test_encrypted_entries_do_not_contain_plaintext() {
        let mut store = EncryptedHistoryStore::new().expect("テスト用履歴ストアの生成に失敗");
        let secret = "super-secret-password-12345";
        store.add(secret, 10);

        assert_eq!(store.entries.len(), 1);
        assert_ne!(store.entries[0].ciphertext.as_slice(), secret.as_bytes());
        assert_eq!(store.entry_at(0).as_ref().map(|s| s.as_str()), Some(secret));
    }

    /// 追加・復号・重複移動・上限が正しく動作すること
    #[test]
    fn test_add_dedup_limit_and_decrypt() {
        let mut store = EncryptedHistoryStore::new().expect("テスト用履歴ストアの生成に失敗");

        // 空白は無視
        store.add("   ", 10);
        assert_eq!(store.len(), 0);

        // 重複するエントリは先頭に移動する
        store.add("first", 10);
        store.add("second", 10);
        store.add("first", 10);

        assert_eq!(
            store.entry_at(0).as_ref().map(|s| s.as_str()),
            Some("first")
        );
        assert_eq!(
            store.entry_at(1).as_ref().map(|s| s.as_str()),
            Some("second")
        );

        // 上限を超えた分は切り捨てられる
        for i in 0..7 {
            store.add(&format!("item-{i}"), 5);
        }
        assert_eq!(store.len(), 5);
        assert_eq!(
            store.entry_at(0).as_ref().map(|s| s.as_str()),
            Some("item-6")
        );

        assert_eq!(
            store.entry_at(0).as_ref().map(|s| s.as_str()),
            Some("item-6")
        );
        assert_eq!(store.entry_at(99), None);

        // clear で履歴が空になること
        store.clear();
        assert_eq!(store.len(), 0);
    }
}
