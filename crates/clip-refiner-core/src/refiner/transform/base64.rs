use base64::Engine as _;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};

// ======================================================================
// デコードエンジン
// ======================================================================
/// パディング (`=`) の有無を問わずデコードするエンジン
///
/// 標準アルファベット (`+` `/`) を使う。URL セーフ形式 (`-` `_`) は呼び出し側で置き換えて渡す
const LENIENT_DECODER: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

// ======================================================================
// Base64 エンコード・デコード
// ======================================================================
/// 文字列を UTF-8 のバイト列として Base64 (標準アルファベット・パディングあり) へエンコードする
///
/// # Arguments
/// * `input` - エンコード対象の文字列
///
/// # Returns
/// * `String` - Base64 文字列
pub fn base64_encode(input: &str) -> String {
    base64_encode_bytes(input.as_bytes())
}

/// バイト列を Base64 (標準アルファベット・パディングあり) へエンコードする
///
/// # Arguments
/// * `input` - エンコード対象のバイト列
///
/// # Returns
/// * `String` - Base64 文字列
pub(crate) fn base64_encode_bytes(input: &[u8]) -> String {
    STANDARD.encode(input)
}

/// Base64 文字列をバイト列へデコードする
///
/// 空白と改行は無視する (MIME のように折り返された Base64 も扱う)
/// 標準 (`+` `/`) と URL セーフ (`-` `_`) のどちらのアルファベットも受け付け、パディングは有無を問わない
/// `data:<MIME>;base64,` で始まる data URI は、先頭の宣言を除いて扱う
///
/// # Arguments
/// * `input` - デコード対象の Base64 文字列
///
/// # Returns
/// * `Option<Vec<u8>>` - デコードされたバイト列。空または Base64 として不正な場合は `None`
pub(crate) fn base64_decode_bytes(input: &str) -> Option<Vec<u8>> {
    let normalized: String = strip_data_uri(input.trim_start())
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    if normalized.is_empty() {
        return None;
    }

    LENIENT_DECODER.decode(normalized).ok()
}

/// Base64 文字列をデコードし、UTF-8 の文字列として返す
///
/// 受け付ける形式は `base64_decode_bytes` と同じ
///
/// # Arguments
/// * `input` - デコード対象の Base64 文字列
///
/// # Returns
/// * `Option<String>` - デコードされた文字列。空・Base64 として不正・デコード結果が UTF-8 として不正な場合は `None`
pub fn base64_decode(input: &str) -> Option<String> {
    String::from_utf8(base64_decode_bytes(input)?).ok()
}

/// `data:<MIME>;base64,` 形式の先頭宣言を取り除く
///
/// 宣言が無い場合、または `;base64` を含まない data URI の場合は入力をそのまま返す
fn strip_data_uri(input: &str) -> &str {
    let Some(rest) = input
        .get(..5)
        .filter(|head| head.eq_ignore_ascii_case("data:"))
        .and(input.get(5..))
    else {
        return input;
    };
    match rest.split_once(',') {
        Some((header, payload)) if header.to_ascii_lowercase().ends_with(";base64") => payload,
        _ => input,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ASCII 文字列を Base64 へエンコードできるようにすること
    #[test]
    fn test_base64_encode_ascii() {
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
        assert_eq!(base64_encode("Man"), "TWFu");
        assert_eq!(base64_encode(""), "");
    }

    /// マルチバイト文字を UTF-8 のバイト列としてエンコードできるようにすること
    #[test]
    fn test_base64_encode_multibyte() {
        assert_eq!(base64_encode("あいう"), "44GC44GE44GG");
    }

    /// 改行コードを変えずにエンコードすること
    #[test]
    fn test_base64_encode_keeps_newlines() {
        assert_eq!(base64_encode("a\r\nb"), "YQ0KYg==");
    }

    /// パディングあり・なしの Base64 をデコードできるようにすること
    #[test]
    fn test_base64_decode_padding() {
        assert_eq!(base64_decode("aGVsbG8=").as_deref(), Some("hello"));
        assert_eq!(base64_decode("aGVsbG8").as_deref(), Some("hello"));
        assert_eq!(base64_decode("TWFu").as_deref(), Some("Man"));
    }

    /// マルチバイト文字をデコードできるようにすること
    #[test]
    fn test_base64_decode_multibyte() {
        assert_eq!(base64_decode("44GC44GE44GG").as_deref(), Some("あいう"));
    }

    /// URL セーフ形式 (`-` `_`) をデコードできるようにすること
    #[test]
    fn test_base64_decode_url_safe() {
        // 標準形式では "??>" が `Pz8+`、"???" が `Pz8/` になる
        assert_eq!(base64_decode("Pz8-").as_deref(), Some("??>"));
        assert_eq!(base64_decode("Pz8_").as_deref(), Some("???"));
        assert_eq!(base64_decode("Pz8+").as_deref(), Some("??>"));
        assert_eq!(base64_decode("Pz8/").as_deref(), Some("???"));
    }

    /// data URI の宣言を取り除いてデコードできるようにすること
    #[test]
    fn test_base64_decode_data_uri() {
        assert_eq!(
            base64_decode("data:text/plain;base64,aGVsbG8=").as_deref(),
            Some("hello")
        );
        assert_eq!(
            base64_decode("  DATA:text/plain;charset=utf-8;BASE64,aGVsbG8=").as_deref(),
            Some("hello")
        );
        // base64 指定のない data URI は対象外
        assert_eq!(base64_decode("data:text/plain,hello"), None);
    }

    /// 空白と改行を無視してデコードできるようにすること
    #[test]
    fn test_base64_decode_ignores_whitespace() {
        assert_eq!(base64_decode("  aGVs\r\nbG8=\n").as_deref(), Some("hello"));
        assert_eq!(base64_decode("aGVs bG8 =").as_deref(), Some("hello"));
    }

    /// Base64 として不正な入力は `None` を返すようにすること
    #[test]
    fn test_base64_decode_invalid() {
        assert_eq!(base64_decode(""), None);
        assert_eq!(base64_decode("  \n"), None);
        assert_eq!(base64_decode("hello world!"), None);
        assert_eq!(base64_decode("a"), None);
        assert_eq!(base64_decode("あいう"), None);
    }

    /// デコード結果が UTF-8 として不正な場合は `None` を返すようにすること
    #[test]
    fn test_base64_decode_invalid_utf8() {
        // 0xFF 0xFE
        assert_eq!(base64_decode("//4="), None);
    }

    /// エンコード結果をデコードして元に戻せるようにすること
    #[test]
    fn test_base64_roundtrip() {
        let original = "user+tag@example.com あいう\r\n🙂 a&b=c/d";
        assert_eq!(
            base64_decode(&base64_encode(original)).as_deref(),
            Some(original)
        );
    }
}
