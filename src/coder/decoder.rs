use percent_encoding::percent_decode_str;

use anyhow::Result;

/// 文字列をパーセントデコードする
///
/// # Arguments
///
/// * `input` - パーセントエンコードされた文字列
///
/// # Returns
///
/// * `Result<String>` - デコードされた文字列
pub fn percent_decode_text(input: &str) -> Result<String> {
    let decoded = percent_decode_str(input).decode_utf8()?;
    Ok(decoded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // アルファベットと数字はそのまま
    fn test_percent_decode_text_alphanumeric() {
        assert_eq!(percent_decode_text("abc123").unwrap(), "abc123");
    }

    #[test]
    // 記号はデコード
    fn test_percent_decode_text_symbols() {
        assert_eq!(percent_decode_text("%20").unwrap(), " ");
        assert_eq!(percent_decode_text("%22").unwrap(), "\"");
        assert_eq!(percent_decode_text("%23").unwrap(), "#");
        assert_eq!(percent_decode_text("%25").unwrap(), "%");
    }

    #[test]
    // 多字節文字はデコード
    fn test_percent_decode_text_multibyte() {
        assert_eq!(
            percent_decode_text("%E3%81%82%E3%81%84%E3%81%86").unwrap(),
            "あいう"
        );
    }

    #[test]
    // 無効なUTF-8はエラー
    fn test_percent_decode_text_bad_utf8() {
        assert!(percent_decode_text("%FF").is_err());
    }
}
