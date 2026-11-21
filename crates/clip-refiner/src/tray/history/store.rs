//! セッション限定の暗号化クリップボード履歴をメモリ上で保持する
//!
//! 起動時に生成した鍵で履歴を暗号化し、プロセス終了時に鍵とデータを破棄する

use crate::security::{
    CIPHER_KEY_LEN, ContentFingerprint, SecretString, decrypt_bytes, encrypt_bytes,
    generate_cipher_key, zeroize_cipher_key,
};

use anyhow::Result;

// ======================================================================
// 暗号化エントリ
// ======================================================================
/// メモリ上に保持する暗号化済み履歴エントリ
struct EncryptedEntry {
    /// 重複判定用のコンテンツ指紋 (平文は保持しない)
    fingerprint: ContentFingerprint,
    /// `encrypt_bytes` 形式の暗号文 (`nonce || ciphertext`)
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
    key: [u8; CIPHER_KEY_LEN],
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
        Ok(Self {
            key: generate_cipher_key()?,
            entries: Vec::new(),
        })
    }
}

// ======================================================================
// 履歴の読み取り
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
// 履歴の書き込み
// ======================================================================
impl EncryptedHistoryStore {
    /// 履歴を追加する
    ///
    /// 空文字は無視し、同一内容 (指紋一致) は先頭へ移動する
    ///
    /// # Arguments
    /// * `text` - 追加する履歴テキスト
    /// * `limit` - 履歴の最大保持数
    pub fn add(&mut self, text: &str, limit: usize) {
        if text.trim().is_empty() {
            return;
        }

        let fingerprint = ContentFingerprint::from_text(text);

        if let Some(pos) = self
            .entries
            .iter()
            .position(|entry| entry.fingerprint == fingerprint)
        {
            self.entries.remove(pos);
        }

        let Some(ciphertext) = self.encrypt(text) else {
            return;
        };

        self.entries.insert(
            0,
            EncryptedEntry {
                fingerprint,
                ciphertext,
            },
        );

        if self.entries.len() > limit {
            self.entries.truncate(limit);
        }
    }

    /// FIFO キューへ末尾に積む (重複許可)
    ///
    /// インデックス 0 が先頭 (最も古いエントリ)
    ///
    /// # Arguments
    /// * `text` - 追加するテキスト
    /// * `limit` - 最大保持数
    pub fn enqueue(&mut self, text: &str, limit: usize) {
        if text.trim().is_empty() {
            return;
        }

        let fingerprint = ContentFingerprint::from_text(text);

        let Some(ciphertext) = self.encrypt(text) else {
            return;
        };

        self.entries.push(EncryptedEntry {
            fingerprint,
            ciphertext,
        });

        if self.entries.len() > limit {
            let overflow = self.entries.len() - limit;
            self.entries.drain(0..overflow);
        }
    }

    /// キュー先頭を取り出して返す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 先頭エントリ。空の場合は `None`
    pub fn discharge_front(&mut self) -> Option<SecretString> {
        if self.entries.is_empty() {
            return None;
        }
        let entry = self.entries.remove(0);
        self.decrypt_entry(&entry).ok()
    }

    /// 先頭から一致する最初のエントリを取り出す
    ///
    /// # Arguments
    /// * `text` - 排出対象の本文
    ///
    /// # Returns
    /// * `bool` - 一致するエントリを除去できた場合は `true`
    #[cfg(test)]
    pub fn discharge_matching(&mut self, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        let target = ContentFingerprint::from_text(text);
        let Some(pos) = self
            .entries
            .iter()
            .position(|entry| entry.fingerprint == target)
        else {
            return false;
        };
        self.entries.remove(pos);
        true
    }

    /// 指定インデックスのエントリを取り出す
    ///
    /// # Arguments
    /// * `index` - キュー内インデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 排出した本文。範囲外の場合は `None`
    pub fn discharge_at(&mut self, index: usize) -> Option<SecretString> {
        if index >= self.entries.len() {
            return None;
        }
        let entry = self.entries.remove(index);
        self.decrypt_entry(&entry).ok()
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
    /// 平文を暗号化する
    ///
    /// # Arguments
    /// * `text` - 暗号化対象のテキスト
    ///
    /// # Returns
    /// * `Option<Vec<u8>>` - 成功時は `encrypt_bytes` 形式の暗号文、失敗時は `None`
    fn encrypt(&self, text: &str) -> Option<Vec<u8>> {
        encrypt_bytes(&self.key, text.as_bytes())
            .inspect_err(|e| {
                crate::log_error!("履歴の暗号化に失敗: {e:#}");
            })
            .ok()
    }

    /// 暗号化エントリを復号する
    ///
    /// # Arguments
    /// * `entry` - 復号対象の暗号化エントリ
    ///
    /// # Returns
    /// * `Result<SecretString, String>` - 成功時は復号済みテキスト、失敗時はエラーメッセージ
    fn decrypt_entry(&self, entry: &EncryptedEntry) -> Result<SecretString, String> {
        let plaintext = decrypt_bytes(&self.key, &entry.ciphertext)
            .map_err(|e| format!("履歴の復号に失敗: {e:#}"))?;
        String::from_utf8(plaintext)
            .map(SecretString::from)
            .map_err(|e| format!("履歴の UTF-8 変換に失敗: {e:?}"))
    }
}

// ======================================================================
// リソース解放
// ======================================================================
impl Drop for EncryptedHistoryStore {
    fn drop(&mut self) {
        zeroize_cipher_key(&mut self.key);
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

    /// FIFO キューで積み・排出が正しく動作すること
    #[test]
    fn test_enqueue_discharge_fifo_queue() {
        let mut store = EncryptedHistoryStore::new().expect("テスト用履歴ストアの生成に失敗");

        store.enqueue("first", 10);
        store.enqueue("second", 10);
        store.enqueue("first", 10);

        assert_eq!(store.len(), 3);
        assert_eq!(
            store.entry_at(0).as_ref().map(|s| s.as_str()),
            Some("first")
        );
        assert_eq!(
            store.entry_at(1).as_ref().map(|s| s.as_str()),
            Some("second")
        );
        assert_eq!(
            store.entry_at(2).as_ref().map(|s| s.as_str()),
            Some("first")
        );

        assert!(store.discharge_matching("second"));
        assert_eq!(store.len(), 2);
        assert_eq!(
            store.entry_at(0).as_ref().map(|s| s.as_str()),
            Some("first")
        );
        assert_eq!(
            store.entry_at(1).as_ref().map(|s| s.as_str()),
            Some("first")
        );

        assert_eq!(
            store.discharge_front().as_ref().map(|s| s.as_str()),
            Some("first")
        );
        assert_eq!(
            store.discharge_front().as_ref().map(|s| s.as_str()),
            Some("first")
        );
        assert!(store.discharge_front().is_none());

        store.enqueue("only", 10);
        assert_eq!(
            store.discharge_at(0).as_ref().map(|s| s.as_str()),
            Some("only")
        );
    }
}
