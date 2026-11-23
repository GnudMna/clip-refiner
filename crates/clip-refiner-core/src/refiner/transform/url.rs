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
/// 英数字と `-` `.` `_` `~` 以外の文字を `%XX` 形式に変換する。空白は `%20` にする
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
/// `%XX` 形式の記述を元の文字に戻す。結果はUTF-8として解釈される
/// `+` はメールアドレスの `user+tag@example.com` などで文字そのものとして使われるため、そのまま残す
///
/// # Arguments
/// * `input` - デコード対象の文字列
///
/// # Returns
/// * `Result<String, std::str::Utf8Error>` - デコードされた文字列
///
/// # Errors
/// デコード結果が UTF-8 として不正な場合 (`%ZZ` のような不正なパーセントエンコードはエラーにせずそのまま残す)
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
/// 対象は URL の `?` のみで、`What?utm_source=1` のような URL でない文字列は変更しない (`is_url_like` を参照)
/// クエリの終わりは、空白・日本語の句読点・対応する `(` のない `)`・`>` `<` `"` `'` `` ` `` のいずれかが現れた位置とし、末尾の句読点は含めない
/// 非 ASCII 文字は、`?q=日本&...` のようにクエリの値として続く場合だけクエリに含め、`xを参照` のような URL 直後の文章は含めない
/// 対象は `utm_` で始まるキーと、広告のクリックID (`fbclid` / `gclid` など)
/// キーは大文字・小文字を区別しない
/// 他のパラメータと、URL以外の文字列・改行・空白は維持される
///
/// # Arguments
/// * `input` - 対象のテキスト (URL単体、URLを含む文章、URLの一覧など)
///
/// # Returns
/// * `Cow<'_, str>` - UTMパラメータが除去されたテキスト。変更がない場合は元の文字列への参照を返す
pub fn remove_utm_params(input: &str) -> Cow<'_, str> {
    let mut output: Option<String> = None;
    // `input` のうち `output` へ未コピーの範囲の開始位置
    let mut copied = 0;
    let mut search = 0;

    while let Some(offset) = input[search..].find('?') {
        let start = search + offset;
        let end = query_end(input, start + 1);
        // クエリは `#` の手前まで。フラグメント内の `?` は次の探索で別のクエリとして扱う
        let query_stop = input[start + 1..end]
            .find('#')
            .map_or(end, |hash| start + 1 + hash);
        if is_url_like(&input[..start])
            && let Some(cleaned) = remove_utm_from_query(&input[start + 1..query_stop])
        {
            let output = output.get_or_insert_with(|| String::with_capacity(input.len()));
            output.push_str(&input[copied..start]);
            output.push_str(&cleaned);
            copied = query_stop;
        }
        search = query_stop;
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

/// URL の区切りとみなす日本語・全角の句読点・括弧か判定する
fn is_wide_punctuation(c: char) -> bool {
    matches!(
        c,
        '\u{3000}'..='\u{303F}'
            | '\u{FF01}'..='\u{FF0F}'
            | '\u{FF1A}'..='\u{FF20}'
            | '\u{FF3B}'..='\u{FF40}'
            | '\u{FF5B}'..='\u{FF65}'
    )
}

/// URL の一部とみなさない文字か判定する (`)` は対応する `(` の有無で別に判定する)
fn is_url_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(c, '>' | '<' | '"' | '\'' | '`') || is_wide_punctuation(c)
}

/// `?` の直前までのテキストが URL の一部とみなせるか判定する
///
/// `?` を含む単語が `/` を含む (`https://` やパス) か `www.` で始まる場合に URL とみなす
/// `What?utm_source=1` のような通常の文章は対象外
///
/// # Arguments
/// * `before` - 対象の `?` より前のテキスト全体
///
/// # Returns
/// * `bool` - URL の一部とみなせる場合は `true`
fn is_url_like(before: &str) -> bool {
    let word = before.rsplit(is_url_boundary).next().unwrap_or("");
    // 日本語の文章に直接続く `www.` も拾うため、最後の非 ASCII 文字より後ろで判定する
    let ascii_tail = word.rsplit(|c: char| !c.is_ascii()).next().unwrap_or("");
    word.contains('/')
        || ascii_tail
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("www."))
        || is_bare_host(ascii_tail)
}

/// スキームも `/` も `www.` もないホスト名 (`example.com` / `example.com:8080`) か判定する
///
/// 2 つ以上のラベルを `.` で連結し、最後のラベル (TLD) は英字 2 文字以上とする
fn is_bare_host(word: &str) -> bool {
    let host = match word.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => word,
    };
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// クエリ文字列(フラグメントを含む)の終了位置を返す
///
/// # Arguments
/// * `input` - 対象のテキスト全体
/// * `from` - `?` の直後の位置
///
/// # Returns
/// * `usize` - 終了位置 (排他)。文末の句読点は含めない
fn query_end(input: &str, from: usize) -> usize {
    let rest = &input[from..];
    let len = query_len(rest);
    let trimmed = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
    // 末尾の `&amp;` の `;` は句読点ではなく区切りの一部なので含める
    if trimmed.ends_with("&amp") && rest[trimmed.len()..len].starts_with(';') {
        return from + trimmed.len() + 1;
    }
    from + trimmed.len()
}

/// 区切り文字が現れるまでのクエリの長さを返す (句読点のトリム前)
///
/// `(` と対応する `)` は URL の一部とする。非 ASCII 文字の連続は、直前が `=` `&` などの区切り、
/// または直後が `&` `#` `=` の場合だけ URL の一部とし、`xを参照` のような URL 直後の文章は含めない
fn query_len(rest: &str) -> usize {
    let mut depth = 0usize;
    let mut prev: Option<char> = None;
    let mut chars = rest.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return index,
            ')' => depth -= 1,
            _ if is_url_boundary(c) => return index,
            _ if !c.is_ascii() => {
                let run_end = rest[index..]
                    .find(|ch: char| ch.is_ascii() || is_url_boundary(ch))
                    .map_or(rest.len(), |offset| index + offset);
                let after = rest[run_end..].chars().next();
                let continues = matches!(prev, Some('=' | '&' | '?' | '#' | '/' | ';'))
                    || matches!(after, Some('&' | '#' | '='));
                if !continues {
                    return index;
                }
                while chars.peek().is_some_and(|&(next, _)| next < run_end) {
                    chars.next();
                }
                prev = rest[..run_end].chars().next_back();
                continue;
            }
            _ => {}
        }
        prev = Some(c);
    }
    rest.len()
}

/// クエリパラメータのキーが計測用パラメータかを判定する (大文字・小文字を区別しない)
///
/// `%75tm_source` のようにパーセントエンコードされたキーはデコードしてから判定する
fn is_tracking_param(key: &str) -> bool {
    let key = percent_decode_str(key).decode_utf8_lossy();
    let key = key.as_ref();
    key.get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("utm_"))
        || CLICK_ID_PARAMS
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
}

/// `?` の直後のクエリ文字列からUTMパラメータを除去する
///
/// # Arguments
/// * `query` - `?` の直後から `#` の手前までのクエリ文字列
///
/// # Returns
/// * `Option<String>` - 除去した場合は、`?` を含めた置き換え後の文字列 (パラメータがすべて除去された場合は空文字列)
///   除去するものがなければ `None`
fn remove_utm_from_query(query: &str) -> Option<String> {
    let params = split_params(query);
    let is_tracking = |param: &str| is_tracking_param(param.split('=').next().unwrap_or(""));
    if !params.iter().any(|(_, param)| is_tracking(param)) {
        // 全く削除されなかった場合
        return None;
    }

    // 末尾の `&` や `&&` による空のパラメータは、除去後に `?` や `&` だけが残る原因になるため併せて捨てる
    let filtered_params: Vec<(&str, &str)> = params
        .iter()
        .copied()
        .filter(|(_, param)| !param.is_empty() && !is_tracking(param))
        .collect();

    if filtered_params.is_empty() {
        // すべて削除された場合は `?` も消す
        Some(String::new())
    } else {
        // 一部削除された場合は、残したパラメータの直前にあった区切りで連結する (先頭のパラメータには区切りを付けない)
        let mut cleaned = String::from("?");
        for (index, (separator, param)) in filtered_params.iter().enumerate() {
            if index > 0 {
                cleaned.push_str(separator);
            }
            cleaned.push_str(param);
        }
        Some(cleaned)
    }
}

/// クエリ文字列をパラメータに分割する
///
/// 区切りは `&` と、HTML エスケープされた `&amp;` の両方。HTML ソースやメールからコピーした URL に対応する
///
/// # Arguments
/// * `query` - `?` の直後から `#` の手前までのクエリ文字列
///
/// # Returns
/// * `Vec<(&str, &str)>` - (直前の区切り, パラメータ) の一覧。先頭のパラメータの区切りは空文字列
fn split_params(query: &str) -> Vec<(&str, &str)> {
    let mut params = Vec::new();
    let mut separator = "";
    let mut rest = query;
    while let Some(index) = rest.find('&') {
        params.push((separator, &rest[..index]));
        separator = if rest[index..].starts_with("&amp;") {
            "&amp;"
        } else {
            "&"
        };
        rest = &rest[index + separator.len()..];
    }
    params.push((separator, rest));
    params
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 英数字はURLエンコードで変更しないようにすること
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

    /// `+` は空白にせずそのまま残し、`%2B` は `+` に戻すようにすること
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

    /// `url_encode` の結果を `url_decode` で元に戻せるようにすること
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

    /// 不正なUTF-8シーケンスのURLデコードはエラーとすること
    #[test]
    fn test_url_decode_bad_utf8() {
        assert!(url_decode("%FF").is_err());
    }

    /// 基本的なUTMパラメータを削除できるようにすること
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
    }

    /// 末尾に `&` が残るURLでも、`?` や `&` を残さず除去すること
    #[test]
    fn test_remove_utm_params_trailing_ampersand() {
        assert_eq!(
            remove_utm_params("https://e.com/?utm_source=a&"),
            "https://e.com/"
        );
        assert_eq!(
            remove_utm_params("https://e.com/?id=1&utm_source=a&"),
            "https://e.com/?id=1"
        );
        assert_eq!(
            remove_utm_params("https://e.com/?&utm_source=a#top"),
            "https://e.com/#top"
        );
        assert_eq!(
            remove_utm_params("https://e.com/?utm_source=a&amp;"),
            "https://e.com/"
        );
        // 除去するものがなければ、空のパラメータがあっても変更しないこと
        assert!(matches!(
            remove_utm_params("https://e.com/?id=1&"),
            Cow::Borrowed(_)
        ));
    }

    /// URLでない文字列は、`?utm_` を含んでいても変更しないこと
    #[test]
    fn test_remove_utm_params_ignores_non_url() {
        for input in [
            "What?utm_source=1 ok",
            "?utm_source=a",
            "why?fbclid=1",
            "Is this OK?utm_medium=x",
        ] {
            assert!(
                matches!(remove_utm_params(input), Cow::Borrowed(s) if s == input),
                "{input}"
            );
        }
        // URLと文章が混在する場合は、URLだけを対象にすること
        assert_eq!(
            remove_utm_params("What?utm_source=1 see www.a.com?utm_source=x /p?utm_medium=y"),
            "What?utm_source=1 see www.a.com /p"
        );
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

    /// UTMパラメータが他のパラメータと混在していても、UTMパラメータだけを除去できるようにすること
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

    /// クエリ中の非 ASCII 文字や `)` を含むパスでも、後ろの計測用パラメータを除去すること
    #[test]
    fn test_remove_utm_params_non_ascii_and_parens() {
        assert_eq!(
            remove_utm_params("https://a.com/?q=日本&utm_source=x"),
            "https://a.com/?q=日本"
        );
        assert_eq!(
            remove_utm_params("https://a.com/?utm_source=x&q=日本語&id=1"),
            "https://a.com/?q=日本語&id=1"
        );
        assert_eq!(
            remove_utm_params("https://a.com/日本?utm_source=x"),
            "https://a.com/日本"
        );
        assert_eq!(
            remove_utm_params("https://a.com/wiki/Foo_(bar)?utm_source=x"),
            "https://a.com/wiki/Foo_(bar)"
        );
        assert_eq!(
            remove_utm_params("(https://a.com/wiki/Foo_(bar)?id=1&utm_source=x)"),
            "(https://a.com/wiki/Foo_(bar)?id=1)"
        );
        assert_eq!(
            remove_utm_params("詳細はwww.a.com?utm_source=xを参照"),
            "詳細はwww.a.comを参照"
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

    /// HTML エスケープされた `&amp;` を `&` と同じ区切りとして扱い、残すパラメータの区切りは維持すること
    #[test]
    fn test_remove_utm_params_html_escaped_ampersand() {
        // 先頭の計測用パラメータを除去しても、次のキーが `amp;` で壊れないこと
        assert_eq!(
            remove_utm_params("https://a.com/?utm_source=x&amp;id=1"),
            "https://a.com/?id=1"
        );
        // 末尾の計測用パラメータを除去し、余分な `&amp;` を残さないこと
        assert_eq!(
            remove_utm_params("https://a.com/?id=1&amp;utm_source=x"),
            "https://a.com/?id=1"
        );
        // 残すパラメータ同士の `&amp;` は維持すること
        assert_eq!(
            remove_utm_params("https://a.com/?a=1&amp;utm_medium=m&amp;b=2&amp;fbclid=y&amp;c=3"),
            "https://a.com/?a=1&amp;b=2&amp;c=3"
        );
        // 計測用パラメータだけの場合は `?` ごと消すこと
        assert_eq!(
            remove_utm_params("https://a.com/?utm_source=x&amp;utm_medium=y#top"),
            "https://a.com/#top"
        );
        // `&` と `&amp;` が混在しても、それぞれの区切りを維持すること
        assert_eq!(
            remove_utm_params("https://a.com/?a=1&b=2&amp;utm_source=x&c=3"),
            "https://a.com/?a=1&b=2&c=3"
        );
        // 計測用パラメータを含まなければ変更しないこと
        assert!(matches!(
            remove_utm_params("https://a.com/?a=1&amp;b=2"),
            Cow::Borrowed(_)
        ));
    }

    /// スキームも `/` も `www.` もないホスト名のURLでも除去し、通常の文章は変更しないこと
    #[test]
    fn test_remove_utm_params_bare_host() {
        assert_eq!(
            remove_utm_params("example.com?utm_source=x&id=1"),
            "example.com?id=1"
        );
        assert_eq!(
            remove_utm_params("see example.com:8080?utm_source=x"),
            "see example.com:8080"
        );
        assert_eq!(
            remove_utm_params("詳細はa-b.co.jp?fbclid=1を参照"),
            "詳細はa-b.co.jpを参照"
        );
        for input in [
            "Really?utm_source=1",
            "v1.2?utm_source=1",
            "e.g.?utm_source=1",
        ] {
            assert!(
                matches!(remove_utm_params(input), Cow::Borrowed(_)),
                "{input}"
            );
        }
    }

    /// パーセントエンコードされたキーも計測用パラメータとして除去すること
    #[test]
    fn test_remove_utm_params_percent_encoded_key() {
        assert_eq!(
            remove_utm_params("https://a.com/?%75tm_source=x&id=1&%66bclid=y"),
            "https://a.com/?id=1"
        );
        assert_eq!(
            remove_utm_params("https://a.com/?utm%5Fsource=x"),
            "https://a.com/"
        );
    }

    /// 変更するURLがない文章は元の参照をそのまま返すようにすること
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

    /// フラグメント内の `?` 以降のクエリも、通常のクエリの有無に関わらず除去すること
    #[test]
    fn test_remove_utm_params_query_in_fragment() {
        assert_eq!(
            remove_utm_params("https://a.com/#/p?utm_source=x"),
            "https://a.com/#/p"
        );
        // 通常のクエリがあっても、フラグメント内のクエリを見落とさないこと
        assert_eq!(
            remove_utm_params("https://a.com/?a=1#/p?utm_source=x"),
            "https://a.com/?a=1#/p"
        );
        assert_eq!(
            remove_utm_params("https://a.com/?utm_medium=y#/p?id=1&fbclid=x"),
            "https://a.com/#/p?id=1"
        );
        // 除去するものがなければ変更しないこと
        assert!(matches!(
            remove_utm_params("https://a.com/?a=1#/p?b=2"),
            Cow::Borrowed(_)
        ));
    }
}
