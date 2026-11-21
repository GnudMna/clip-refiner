// ======================================================================
// コンテンツ指紋
// ======================================================================
/// クリップボード本文の同一性判定用指紋
///
/// 平文を保持せず BLAKE3 ハッシュとバイト長で比較する
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
    pub fn from_text(text: &str) -> Self {
        Self {
            hash: *blake3::hash(text.as_bytes()).as_bytes(),
            byte_len: text.len(),
        }
    }

    /// テキストがこの指紋と一致するか判定する
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

    /// 異なるテキストは一致しないこと
    #[test]
    fn fingerprint_rejects_different_text() {
        let fp = ContentFingerprint::from_text("hello");
        assert!(!fp.matches("world"));
    }

    /// 長さが異なる場合は一致しないこと
    #[test]
    fn fingerprint_rejects_different_length() {
        let fp = ContentFingerprint::from_text("ab");
        assert!(!fp.matches("abc"));
    }
}
