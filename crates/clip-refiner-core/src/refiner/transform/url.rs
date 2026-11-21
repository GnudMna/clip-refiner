use std::borrow::Cow;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

// ======================================================================
// エンコードセット
// ======================================================================
/// RFC 3986 の非予約文字 (英数字と `-` `.` `_` `~`) 以外を全部エンコードするセット
///
/// `&` `=` `+` `/` などの予約文字も対象とし、クエリの値としてそのまま埋め込めるようにする
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

// ======================================================================
// URL エンコード・デコード
// ======================================================================
/// 文字列をURLエンコード(パーセントエンコーディング)する
///
/// 英数字と `-` `.` `_` `~` 以外の文字を `%XX` 形式に変換する。空白は `%20` にする。
///
/// # Arguments
/// * `input` - エンコード対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - URLエンコードされた文字列
pub fn url_encode(input: &str) -> Cow<'_, str> {
    utf8_percent_encode(input, ENCODE_SET).into()
}

/// URLエンコードされた文字列をデコードする
///
/// `%XX` 形式の記述を元の文字に戻し、`+` を空白に戻す。結果はUTF-8として解釈される。
/// `+` そのものは `%2B` と書く。
///
/// # Arguments
/// * `input` - デコード対象の文字列
///
/// # Returns
/// * `Result<String, std::str::Utf8Error>` - デコードされた文字列
///
/// # Errors
/// 不正なパーセントエンコード、または UTF-8 として不正な場合
pub fn url_decode(input: &str) -> Result<String, std::str::Utf8Error> {
    // フォーム形式 (application/x-www-form-urlencoded) の `+` を空白として扱う
    let input: Cow<'_, str> = if input.contains('+') {
        Cow::Owned(input.replace('+', " "))
    } else {
        Cow::Borrowed(input)
    };
    let decoded = percent_decode_str(&input).decode_utf8()?;
    Ok(decoded.into_owned())
}
// ======================================================================
// UTM パラメータ削除
// ======================================================================
/// URLからUTMパラメータ(計測用パラメータ)を除去する
///
/// URLのクエリ文字列(?以降)に含まれる `utm_` で始まるパラメータをすべて取り除く
/// 他のパラメータは維持される
///
/// # Arguments
/// * `input` - 対象のURL文字列
///
/// # Returns
/// * `Cow<'_, str>` - UTMパラメータが除去されたURL文字列。変更がない場合は元の文字列への参照を返す。
pub fn remove_utm_params(input: &str) -> Cow<'_, str> {
    let mut parts = input.splitn(2, '?');
    let base = parts.next().unwrap_or("");
    let Some(query_and_fragment) = parts.next() else {
        return Cow::Borrowed(input);
    };

    // クエリ文字列とフラグメント(#以降)を分離する
    let (query, fragment) = match query_and_fragment.split_once('#') {
        Some((q, f)) => (q, Some(f)),
        None => (query_and_fragment, None),
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
    } else {
        // フラグメントを復元する
        let fragment_str = fragment.map(|f| format!("#{f}")).unwrap_or_default();
        if filtered_params.is_empty() {
            // すべて削除された場合
            Cow::Owned(format!("{base}{fragment_str}"))
        } else {
            // 一部削除された場合
            Cow::Owned(format!(
                "{}?{}{}",
                base,
                filtered_params.join("&"),
                fragment_str
            ))
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 英数字のURLエンコードできるようにすること
    /// 変更されないことを確認できるようにすること
    #[test]
    fn test_url_encode_alphanumeric() {
        assert_eq!(url_encode("abc123"), "abc123");
    }

    /// 記号のURLエンコードできるようにすること
    #[test]
    fn test_url_encode_symbols() {
        assert_eq!(url_encode(" "), "%20");
        assert_eq!(url_encode("\""), "%22");
    }

    /// 英数字のURLデコードできるようにすること
    #[test]
    fn test_url_decode_alphanumeric() {
        assert_eq!(url_decode("abc123").unwrap(), "abc123");
    }

    /// 記号のURLデコードできるようにすること
    #[test]
    fn test_url_decode_symbols() {
        assert_eq!(url_decode("%20").unwrap(), " ");
    }

    /// `+` を空白に戻し、`%2B` は `+` に戻すようにすること
    #[test]
    fn test_url_decode_plus() {
        assert_eq!(url_decode("foo+bar").unwrap(), "foo bar");
        assert_eq!(url_decode("a%2Bb").unwrap(), "a+b");
        assert_eq!(url_decode("a%26b%3Dc%2Bd%2Fe").unwrap(), "a&b=c+d/e");
    }

    /// マルチバイト文字のURLデコードできるようにすること
    #[test]
    fn test_url_decode_multibyte() {
        assert_eq!(url_decode("%E3%81%82%E3%81%84%E3%81%86").unwrap(), "あいう");
    }

    /// 不正なUTF-8シーケンスのURLデコードできるようにすること
    /// エラーになることを確認できるようにすること
    #[test]
    fn test_url_decode_bad_utf8() {
        assert!(url_decode("%FF").is_err());
    }

    /// UTMパラメータ削除のテスト(基本)できるようにすること
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

    /// スペースや予約文字が含まれる場合のURLエンコードできるようにすること
    #[test]
    fn test_url_encode_space_special() {
        assert_eq!(url_encode("foo bar"), "foo%20bar");
        assert_eq!(url_encode("foo+bar"), "foo%2Bbar");
        assert_eq!(url_encode("a&b=c+d/e"), "a%26b%3Dc%2Bd%2Fe");
        assert_eq!(
            url_encode("!$'()*,:;@[]"),
            "%21%24%27%28%29%2A%2C%3A%3B%40%5B%5D"
        );
    }

    /// 非予約文字 (`-` `.` `_` `~`) はエンコードしないようにすること
    #[test]
    fn test_url_encode_unreserved() {
        assert_eq!(url_encode("a-b.c_d~e"), "a-b.c_d~e");
    }

    /// マルチバイト文字をUTF-8のバイト列としてエンコードできるようにすること
    #[test]
    fn test_url_encode_multibyte() {
        assert_eq!(url_encode("あいう"), "%E3%81%82%E3%81%84%E3%81%86");
    }

    /// UTMパラメータが他のパラメータと混在している場合できるようにすること
    #[test]
    fn test_remove_utm_params_complex() {
        assert_eq!(
            remove_utm_params("http://example.com?a=1&utm_source=s&b=2"),
            "http://example.com?a=1&b=2"
        );
        // utm_ は小文字のみ対象。大文字の UTM_SOURCE は削除しない
        assert_eq!(
            remove_utm_params("http://example.com?UTM_SOURCE=S"),
            "http://example.com?UTM_SOURCE=S"
        );
    }

    /// フラグメント(#)がある場合のUTMパラメータ削除できるようにすること
    #[test]
    fn test_remove_utm_params_with_fragment() {
        // UTMパラメータ削除後もフラグメントが保持されること
        assert_eq!(
            remove_utm_params("https://example.com/?utm_source=google&id=123#section"),
            "https://example.com/?id=123#section"
        );
        // UTMパラメータのみ存在する場合、フラグメントだけが残ること
        assert_eq!(
            remove_utm_params("https://example.com/?utm_source=google#section"),
            "https://example.com/#section"
        );
        // 変更なしの場合はフラグメントごと元の参照を返すこと
        assert_eq!(
            remove_utm_params("https://example.com/?id=123#section"),
            "https://example.com/?id=123#section"
        );
        // フラグメントのみ(クエリなし)はそのまま
        assert_eq!(
            remove_utm_params("https://example.com/#section"),
            "https://example.com/#section"
        );
    }
}
