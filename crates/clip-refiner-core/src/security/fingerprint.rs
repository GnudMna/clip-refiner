use std::sync::OnceLock;

// ======================================================================
// セッション鍵
// ======================================================================
/// 指紋計算に使うプロセス限定のランダム鍵
///
/// 鍵なしハッシュではメモリ上の指紋から PIN などの短い値を総当たりで復元できる
/// 起動ごとに生成した鍵で `blake3::keyed_hash` を使い、鍵を持たない第三者が平文を推測できないようにする
static SESSION_KEY: OnceLock<[u8; blake3::KEY_LEN]> = OnceLock::new();

/// 指紋用セッション鍵を返す
///
/// 初回呼び出し時に OS の乱数で生成する。乱数取得に失敗した場合は全ゼロ鍵に退避する
/// (履歴ストアの鍵生成が先に失敗するため通常は到達しない)
///
/// # Returns
/// * `&'static [u8; blake3::KEY_LEN]` - セッション鍵
fn session_key() -> &'static [u8; blake3::KEY_LEN] {
    SESSION_KEY.get_or_init(|| {
        let mut key = [0u8; blake3::KEY_LEN];
        if getrandom::fill(&mut key).is_err() {
            key = [0u8; blake3::KEY_LEN];
        }
        key
    })
}

// ======================================================================
// コンテンツ指紋
// ======================================================================
/// クリップボード本文の同一性判定用指紋
///
/// 平文を保持せず、セッション鍵付き BLAKE3 ハッシュとバイト長で比較する
/// 鍵はプロセスごとに異なるため、プロセスをまたいで指紋を比較できない
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentFingerprint {
    hash: [u8; 32],
    byte_len: usize,
}

impl Default for ContentFingerprint {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl ContentFingerprint {
    /// テキストから指紋を生成する
    ///
    /// # Arguments
    /// * `text` - 指紋化対象のテキスト
    ///
    /// # Returns
    /// * `Self` - セッション鍵付き BLAKE3 ハッシュとバイト長からなる指紋
    pub fn from_text(text: &str) -> Self {
        Self {
            hash: *blake3::keyed_hash(session_key(), text.as_bytes()).as_bytes(),
            byte_len: text.len(),
        }
    }

    /// テキストがこの指紋と一致するか判定する
    ///
    /// # Arguments
    /// * `self` - 比較対象の指紋
    /// * `text` - 判定対象のテキスト
    ///
    /// # Returns
    /// * `bool` - ハッシュとバイト長が一致する場合は `true`
    pub fn matches(&self, text: &str) -> bool {
        text.len() == self.byte_len && Self::from_text(text).hash == self.hash
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 同一テキストは一致すること
    #[test]
    fn fingerprint_matches_same_text() {
        let fp = ContentFingerprint::from_text("hello");
        assert!(fp.matches("hello"));
    }

    /// 異なるテキストは一致しないようにすること
    #[test]
    fn fingerprint_rejects_different_text() {
        let fp = ContentFingerprint::from_text("hello");
        assert!(!fp.matches("world"));
    }

    /// 長さが異なる場合は一致しないようにすること
    #[test]
    fn fingerprint_rejects_different_length() {
        let fp = ContentFingerprint::from_text("ab");
        assert!(!fp.matches("abc"));
    }

    /// 指紋が鍵なしハッシュと一致しないようにすること
    ///
    /// 鍵なしハッシュと同じ値なら、平文の総当たりで復元できてしまう
    #[test]
    fn fingerprint_is_not_unkeyed_hash() {
        let fp = ContentFingerprint::from_text("123456");
        assert_ne!(fp.hash, *blake3::hash(b"123456").as_bytes());
    }
}
