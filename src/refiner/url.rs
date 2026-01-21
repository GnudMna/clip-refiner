use anyhow::Result;
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

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
pub fn encode(input: &str) -> String {
    utf8_percent_encode(input, ENCODE_SET).to_string()
}

/// 文字列をパーセントデコードする
pub fn decode(input: &str) -> Result<String> {
    let decoded = percent_decode_str(input).decode_utf8()?;
    Ok(decoded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_alphanumeric() {
        assert_eq!(encode("abc123"), "abc123");
    }

    #[test]
    fn test_encode_symbols() {
        assert_eq!(encode(" "), "%20");
        assert_eq!(encode("\""), "%22");
    }

    #[test]
    fn test_decode_alphanumeric() {
        assert_eq!(decode("abc123").unwrap(), "abc123");
    }

    #[test]
    fn test_decode_symbols() {
        assert_eq!(decode("%20").unwrap(), " ");
    }

    #[test]
    fn test_decode_multibyte() {
        assert_eq!(decode("%E3%81%82%E3%81%84%E3%81%86").unwrap(), "あいう");
    }

    #[test]
    fn test_decode_bad_utf8() {
        assert!(decode("%FF").is_err());
    }
}
