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
/// `%XX` 形式の記述を元の文字に戻す。結果はUTF-8として解釈される。
/// `+` はメールアドレスの `user+tag@example.com` などで文字そのものとして使われるため、そのまま残す。
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
    let decoded = percent_decode_str(input).decode_utf8()?;
    Ok(decoded.into_owned())
}

// ======================================================================
// UTM パラメータ削除
// ======================================================================
/// テキストに含まれるURLからUTMパラメータ(計測用パラメータ)を除去する
///
/// `?` から始まるクエリ文字列の計測用パラメータをすべて取り除く
/// クエリの終わりは、空白・ASCII 以外の文字・`)` `>` `<` `"` `'` `` ` `` のいずれかが現れた位置とし、末尾の句読点は含めない
/// 対象は `utm_` で始まるキーと、広告のクリックID (`fbclid` / `gclid` など)
/// キーは大文字・小文字を区別しない
/// 他のパラメータと、URL以外の文字列・改行・空白は維持される
///
/// # Arguments
/// * `input` - 対象のテキスト (URL単体、URLを含む文章、URLの一覧など)
///
/// # Returns
/// * `Cow<'_, str>` - UTMパラメータが除去されたテキスト。変更がない場合は元の文字列への参照を返す。
pub fn remove_utm_params(input: &str) -> Cow<'_, str> {
    let mut output: Option<String> = None;
    // `input` のうち `output` へ未コピーの範囲の開始位置
    let mut copied = 0;
    let mut search = 0;

    while let Some(offset) = input[search..].find('?') {
        let start = search + offset;
        let end = query_end(input, start + 1);
        if let Some(cleaned) = remove_utm_from_query(&input[start + 1..end]) {
            let output = output.get_or_insert_with(|| String::with_capacity(input.len()));
            output.push_str(&input[copied..start]);
            output.push_str(&cleaned);
            copied = end;
        }
        search = end;
    }

    match output {
        Some(mut output) => {
            output.push_str(&input[copied..]);
            Cow::Owned(output)
        }
        None => Cow::Borrowed(input),
    }
}

/// 除去する広告クリックIDのキー (小文字)
const CLICK_ID_PARAMS: &[&str] = &[
    "fbclid", "gclid", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "twclid", "ttclid",
    "igshid",
];

/// クエリの直後に続いていても URL の一部とみなさない文字か判定する
fn is_query_terminator(c: char) -> bool {
    c.is_whitespace() || !c.is_ascii() || matches!(c, ')' | '>' | '<' | '"' | '\'' | '`')
}

/// クエリ文字列(フラグメントを含む)の終了位置を返す
///
/// # Arguments
/// * `input` - 対象のテキスト全体
/// * `from` - `?` の直後の位置
///
/// # Returns
/// * `usize` - 終了位置 (排他)。文末の句読点は含めない。
fn query_end(input: &str, from: usize) -> usize {
    let rest = &input[from..];
    let len = rest.find(is_query_terminator).unwrap_or(rest.len());
    let trimmed = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
    from + trimmed.len()
}

/// クエリパラメータのキーが計測用パラメータかを判定する (大文字・小文字を区別しない)
fn is_tracking_param(key: &str) -> bool {
    key.get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("utm_"))
        || CLICK_ID_PARAMS
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
}

/// `?` の直後のクエリ文字列からUTMパラメータを除去する
///
/// # Arguments
/// * `query_and_fragment` - `?` の直後から始まるクエリ文字列 (`#` 以降のフラグメントを含む)
///
/// # Returns
/// * `Option<String>` - 除去した場合は、`?` を含めた置き換え後の文字列。除去するものがなければ `None`。
fn remove_utm_from_query(query_and_fragment: &str) -> Option<String> {
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
            !is_tracking_param(key)
        })
        .collect();

    if params.len() == filtered_params.len() {
        // 全く削除されなかった場合
        return None;
    }

    // フラグメントを復元する
    let fragment_str = fragment.map(|f| format!("#{f}")).unwrap_or_default();
    if filtered_params.is_empty() {
        // すべて削除された場合は `?` も消す
        Some(fragment_str)
    } else {
        // 一部削除された場合
        Some(format!("?{}{}", filtered_params.join("&"), fragment_str))
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

    /// `+` は空白にせずそのまま残し、`%2B` は `+` に戻すこと
    #[test]
    fn test_url_decode_plus() {
        assert_eq!(url_decode("foo+bar").unwrap(), "foo+bar");
        assert_eq!(
            url_decode("user+tag%40example.com").unwrap(),
            "user+tag@example.com"
        );
        assert_eq!(url_decode("a%2Bb").unwrap(), "a+b");
        assert_eq!(url_decode("a%26b%3Dc%2Bd%2Fe").unwrap(), "a&b=c+d/e");
    }

    /// `url_encode` の結果を `url_decode` で元に戻せること
    #[test]
    fn test_url_encode_decode_roundtrip() {
        let original = "user+tag@example.com a&b=c/d";
        assert_eq!(url_decode(&url_encode(original)).unwrap(), original);
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
    }

    /// キーの大文字・小文字を区別せず utm_ パラメータを除去すること
    #[test]
    fn test_remove_utm_params_case_insensitive() {
        assert_eq!(
            remove_utm_params("http://example.com?UTM_SOURCE=S&Utm_Medium=m&id=1"),
            "http://example.com?id=1"
        );
        // utm_ で始まらないキーは残す
        assert_eq!(
            remove_utm_params("http://example.com?utm=1&xutm_source=2"),
            "http://example.com?utm=1&xutm_source=2"
        );
        // マルチバイト文字で始まるキーでも panic しないこと
        assert_eq!(
            remove_utm_params("http://example.com?あutm=1"),
            "http://example.com?あutm=1"
        );
    }

    /// 広告のクリックID (fbclid / gclid など) を除去すること
    #[test]
    fn test_remove_utm_params_click_ids() {
        assert_eq!(
            remove_utm_params("https://example.com/?id=1&fbclid=abc&gclid=def"),
            "https://example.com/?id=1"
        );
        assert_eq!(
            remove_utm_params("https://example.com/?FBCLID=abc&MsClkId=x#top"),
            "https://example.com/#top"
        );
        // 完全一致のみ対象。前方一致や部分一致のキーは残す
        assert_eq!(
            remove_utm_params("https://example.com/?fbclid_x=1&my_gclid=2"),
            "https://example.com/?fbclid_x=1&my_gclid=2"
        );
    }

    /// URLを含む文章でも、URL以外の文字列を失わずUTMパラメータだけを除去すること
    #[test]
    fn test_remove_utm_params_keeps_surrounding_text() {
        assert_eq!(
            remove_utm_params("see https://a.com/?utm_source=x and https://b.com/?id=1"),
            "see https://a.com/ and https://b.com/?id=1"
        );
        assert_eq!(
            remove_utm_params("a https://a.com/?id=1&utm_source=x b"),
            "a https://a.com/?id=1 b"
        );
    }

    /// 複数行のURL一覧で、各行を個別に処理し改行を維持すること
    #[test]
    fn test_remove_utm_params_multiline() {
        assert_eq!(
            remove_utm_params("https://a.com/?utm_source=x\nhttps://b.com/?id=1"),
            "https://a.com/\nhttps://b.com/?id=1"
        );
        assert_eq!(
            remove_utm_params("https://a.com/?utm_source=x\r\n\r\n  https://b.com/?utm_medium=y\n"),
            "https://a.com/\r\n\r\n  https://b.com/\n"
        );
    }

    /// URLの直後に空白なしで続く文字を、URLの一部とみなさず維持すること
    #[test]
    fn test_remove_utm_params_keeps_adjacent_text() {
        // 日本語の文章中
        assert_eq!(
            remove_utm_params("詳細はhttps://a.com/?utm_source=xを参照"),
            "詳細はhttps://a.com/を参照"
        );
        assert_eq!(
            remove_utm_params("https://a.com/?id=1&utm_source=x、https://b.com/?fbclid=y。"),
            "https://a.com/?id=1、https://b.com/。"
        );
        // 括弧・引用符・山括弧・バッククォートで囲まれたURL
        assert_eq!(
            remove_utm_params("(https://a.com/?utm_source=x)"),
            "(https://a.com/)"
        );
        assert_eq!(
            remove_utm_params("[t](https://a.com/?id=1&utm_source=x)"),
            "[t](https://a.com/?id=1)"
        );
        assert_eq!(
            remove_utm_params("<https://a.com/?utm_source=x>"),
            "<https://a.com/>"
        );
        assert_eq!(
            remove_utm_params("\"https://a.com/?utm_source=x\" 'https://b.com/?utm_medium=y'"),
            "\"https://a.com/\" 'https://b.com/'"
        );
        assert_eq!(
            remove_utm_params("`https://a.com/?utm_source=x`"),
            "`https://a.com/`"
        );
        // フラグメントも終端まで保持する
        assert_eq!(
            remove_utm_params("(https://a.com/?utm_source=x#top)"),
            "(https://a.com/#top)"
        );
    }

    /// 末尾の句読点をクエリの値に含めず維持すること
    #[test]
    fn test_remove_utm_params_keeps_trailing_punctuation() {
        assert_eq!(
            remove_utm_params("see https://a.com/?id=1&utm_source=x."),
            "see https://a.com/?id=1."
        );
        assert_eq!(
            remove_utm_params("https://a.com/?utm_source=x, https://b.com/?utm_medium=y!"),
            "https://a.com/, https://b.com/!"
        );
        // 句読点を値の途中に含むパラメータは削除せず維持する
        assert_eq!(
            remove_utm_params("https://a.com/?id=1.5&q=a,b"),
            "https://a.com/?id=1.5&q=a,b"
        );
    }

    /// 変更するURLがない文章は元の参照をそのまま返すこと
    #[test]
    fn test_remove_utm_params_unchanged_text_is_borrowed() {
        let input = "see https://b.com/?id=1\n  and more";
        assert!(matches!(remove_utm_params(input), Cow::Borrowed(_)));
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
