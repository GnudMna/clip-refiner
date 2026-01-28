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

/// 文字列をURLエンコードする
///
/// # Arguments
/// * `input` - エンコードする文字列。
///
/// # Returns
/// * `String` - URLエンコードされた文字列。
pub fn url_encode(input: &str) -> String {
    utf8_percent_encode(input, ENCODE_SET).to_string()
}

/// 文字列をURLデコードする
///
/// # Arguments
/// * `input` - デコードする文字列。
///
/// # Returns
/// * `Result<String>` - デコードされた文字列。デコードに失敗した場合は `Err` を返す。
pub fn url_decode(input: &str) -> Result<String> {
    let decoded = percent_decode_str(input).decode_utf8()?;
    Ok(decoded.into_owned())
}

/// URLからUTMパラメータを除去する
///
/// # Arguments
/// * `input` - 対象のURL文字列。
///
/// # Returns
/// * `String` - UTMパラメータが除去されたURL文字列。
pub fn remove_utm_params(input: &str) -> String {
    let mut parts = input.splitn(2, '?');
    let base = parts.next().unwrap_or("");
    let query = match parts.next() {
        Some(q) => q,
        None => return input.to_string(),
    };

    let filtered_query: Vec<&str> = query
        .split('&')
        .filter(|param| {
            let key = param.split('=').next().unwrap_or("");
            !key.starts_with("utm_")
        })
        .collect();

    if filtered_query.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, filtered_query.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode_alphanumeric() {
        assert_eq!(url_encode("abc123"), "abc123");
    }

    #[test]
    fn test_url_encode_symbols() {
        assert_eq!(url_encode(" "), "%20");
        assert_eq!(url_encode("\""), "%22");
    }

    #[test]
    fn test_url_decode_alphanumeric() {
        assert_eq!(url_decode("abc123").unwrap(), "abc123");
    }

    #[test]
    fn test_url_decode_symbols() {
        assert_eq!(url_decode("%20").unwrap(), " ");
    }

    #[test]
    fn test_url_decode_multibyte() {
        assert_eq!(url_decode("%E3%81%82%E3%81%84%E3%81%86").unwrap(), "あいう");
    }

    #[test]
    fn test_url_decode_bad_utf8() {
        assert!(url_decode("%FF").is_err());
    }

    #[test]
    fn test_remove_utm_params() {
        assert_eq!(
            remove_utm_params("https://example.com/?utm_source=google&utm_medium=cpc&id=123"),
            "https://example.com/?id=123"
        );
        assert_eq!(
            remove_utm_params("https://example.com/?utm_source=google"),
            "https://example.com/"
        );
        assert_eq!(
            remove_utm_params("https://example.com/path?a=b&utm_campaign=xyz&c=d"),
            "https://example.com/path?a=b&c=d"
        );
        assert_eq!(
            remove_utm_params("https://example.com/"),
            "https://example.com/"
        );
        assert_eq!(
            remove_utm_params("https://example.com/?utm_content=test&foo=bar"),
            "https://example.com/?foo=bar"
        );
        assert_eq!(remove_utm_params("?utm_source=a"), "");
    }
}
