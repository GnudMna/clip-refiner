use std::borrow::Cow;

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

// ======================================================================
// URL エンコード・デコード
// ======================================================================
/// 文字列をURLエンコード（パーセントエンコーディング）する
///
/// 入力文字列内の特殊文字を `%XX` 形式に変換します。
/// 英数字以外の多くの文字がエンコード対象となります。
///
/// # Arguments
/// * `input` - エンコード対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - URLエンコードされた文字列。
pub fn url_encode(input: &str) -> Cow<'_, str> {
    utf8_percent_encode(input, ENCODE_SET).into()
}

/// URLエンコードされた文字列をデコードする
///
/// `%XX` 形式の記述を元の文字に戻します。結果はUTF-8として解釈されます。
///
/// # Arguments
/// * `input` - デコード対象の文字列
///
/// # Returns
/// * `Result<String>` - デコードされた文字列。不正なエンコードやUTF-8として不正な場合は `Err` を返します。
pub fn url_decode(input: &str) -> Result<String> {
    let decoded = percent_decode_str(input).decode_utf8()?;
    Ok(decoded.into_owned())
}
// ======================================================================
// UTM パラメータ削除
// ======================================================================
/// URLからUTMパラメータ（計測用パラメータ）を除去する
///
/// URLのクエリ文字列（?以降）に含まれる `utm_` で始まるパラメータをすべて取り除きます。
/// 他のパラメータは維持されます。
///
/// # Arguments
/// * `input` - 対象のURL文字列
///
/// # Returns
/// * `Cow<'_, str>` - UTMパラメータが除去されたURL文字列。変更がない場合は元の文字列への参照を返します。
pub fn remove_utm_params(input: &str) -> Cow<'_, str> {
    let mut parts = input.splitn(2, '?');
    let base = parts.next().unwrap_or("");
    let query = match parts.next() {
        Some(q) => q,
        None => return Cow::Borrowed(input),
    };

    let params: Vec<&str> = query.split('&').collect();
    let filtered_params: Vec<&str> = params
        .iter()
        .copied()
        .filter(|param| {
            let key = param.split('=').next().unwrap_or("");
            !key.starts_with("utm_")
        })
        .collect();

    if params.len() == filtered_params.len() {
        // 全く削除されなかった場合
        Cow::Borrowed(input)
    } else if filtered_params.is_empty() {
        // すべて削除された場合
        Cow::Owned(base.to_string())
    } else {
        // 一部削除された場合
        Cow::Owned(format!("{}?{}", base, filtered_params.join("&")))
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 英数字のURLエンコードテスト
    /// 変更されないことを確認
    #[test]
    fn test_url_encode_alphanumeric() {
        assert_eq!(url_encode("abc123"), "abc123");
    }

    /// 記号のURLエンコードテスト
    #[test]
    fn test_url_encode_symbols() {
        assert_eq!(url_encode(" "), "%20");
        assert_eq!(url_encode("\""), "%22");
    }

    /// 英数字のURLデコードテスト
    #[test]
    fn test_url_decode_alphanumeric() {
        assert_eq!(url_decode("abc123").unwrap(), "abc123");
    }

    /// 記号のURLデコードテスト
    #[test]
    fn test_url_decode_symbols() {
        assert_eq!(url_decode("%20").unwrap(), " ");
    }

    /// マルチバイト文字のURLデコードテスト
    #[test]
    fn test_url_decode_multibyte() {
        assert_eq!(url_decode("%E3%81%82%E3%81%84%E3%81%86").unwrap(), "あいう");
    }

    /// 不正なUTF-8シーケンスのURLデコードテスト
    /// エラーになることを確認
    #[test]
    fn test_url_decode_bad_utf8() {
        assert!(url_decode("%FF").is_err());
    }

    /// UTMパラメータ削除のテスト(基本)
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

    /// スペースや特殊文字が含まれる場合のURLエンコードテスト
    /// プラス記号などはENCODE_SETに含まれないためそのまま残る挙動などを確認
    #[test]
    fn test_url_encode_space_special() {
        assert_eq!(url_encode("foo bar"), "foo%20bar");
        // ENCODE_SET does not include '+', so it remains as '+'
        assert_eq!(url_encode("foo+bar"), "foo+bar");
    }

    /// UTMパラメータが他のパラメータと混在している場合のテスト
    #[test]
    fn test_remove_utm_params_complex() {
        assert_eq!(
            remove_utm_params("http://example.com?a=1&utm_source=s&b=2"),
            "http://example.com?a=1&b=2"
        );
        // Case sensitive check? utm_ is usually lowercase.
        // My implementation assumes lowercase "utm_".
        assert_eq!(
            remove_utm_params("http://example.com?UTM_SOURCE=S"),
            "http://example.com?UTM_SOURCE=S"
        );
    }
}
