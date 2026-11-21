use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// 「ASCII の制御文字 + 非英数字」を全部エンコードするセット
const ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

/// 文字列をパーセントエンコードする
///
/// # Arguments
///
/// * `input` - エンコードする文字列
///
/// # Returns
///
/// * `String` - エンコードされた文字列
pub fn percent_encode_text(input: &str) -> String {
    utf8_percent_encode(input, ENCODE_SET).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // アルファベットと数字はそのまま
    fn test_percent_encode_text_alphanumeric() {
        assert_eq!(percent_encode_text("abc123"), "abc123");
    }

    #[test]
    // 記号はエンコード
    fn test_percent_encode_text_symbols() {
        assert_eq!(percent_encode_text(" "), "%20");
        assert_eq!(percent_encode_text("\""), "%22");
        assert_eq!(percent_encode_text("#"), "%23");
        assert_eq!(percent_encode_text("%"), "%25");
        assert_eq!(percent_encode_text("<"), "%3C");
        assert_eq!(percent_encode_text(">"), "%3E");
        assert_eq!(percent_encode_text("?"), "%3F");
        assert_eq!(percent_encode_text("`"), "%60");
        assert_eq!(percent_encode_text("{"), "%7B");
        assert_eq!(percent_encode_text("}"), "%7D");
    }

    #[test]
    // 多字節文字はエンコード
    fn test_percent_encode_text_multibyte() {
        assert_eq!(percent_encode_text("あいう"), "%E3%81%82%E3%81%84%E3%81%86");
    }

    #[test]
    // 混合はエンコード
    fn test_percent_encode_text_mixed() {
        assert_eq!(percent_encode_text("a b"), "a%20b");
    }
}
