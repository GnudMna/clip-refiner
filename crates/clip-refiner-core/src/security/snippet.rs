use std::sync::LazyLock;

use crate::consts::SENSITIVE_SNIPPET_LABEL;

use regex::Regex;

// ======================================================================
// 機密情報の検出
// ======================================================================
/// 機密情報検出用の正規表現をコンパイルする
#[allow(clippy::expect_used)]
fn sensitive_regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("機密情報検出用正規表現のコンパイルに失敗")
}

static PEM_KEY: LazyLock<Regex> =
    LazyLock::new(|| sensitive_regex(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"));

static JWT: LazyLock<Regex> =
    LazyLock::new(|| sensitive_regex(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\."));

static AWS_ACCESS_KEY: LazyLock<Regex> = LazyLock::new(|| sensitive_regex(r"\bAKIA[0-9A-Z]{16}\b"));

static TOKEN_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    sensitive_regex(
        r"(?i)\b(?:sk|pk)_(?:live|test)_[A-Za-z0-9]{10,}\b|\bghp_[A-Za-z0-9]{20,}\b|\bgho_[A-Za-z0-9]{20,}\b|\bgithub_pat_[A-Za-z0-9_]{20,}\b",
    )
});

static CREDENTIAL_LINE: LazyLock<Regex> = LazyLock::new(|| {
    sensitive_regex(
        r"(?im)^[^\n#]{0,40}(?:password|passwd|secret|token|api[_-]?key|authorization|credential)\s*[:=]\s*\S+",
    )
});

static BARE_SECRET_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| sensitive_regex(r"^[A-Za-z0-9+/=_-]{32,}$"));

/// テキストが機密情報を含む可能性があるか判定する
///
/// 誤検知を避けるため、既知パターンと資格情報らしい行形式のみを対象とする
pub fn looks_sensitive(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    if PEM_KEY.is_match(trimmed)
        || JWT.is_match(trimmed)
        || AWS_ACCESS_KEY.is_match(trimmed)
        || TOKEN_PREFIX.is_match(trimmed)
        || CREDENTIAL_LINE.is_match(trimmed)
    {
        return true;
    }

    // 単一行の裸のトークン (URL や通常文は除外)
    trimmed.lines().count() == 1 && BARE_SECRET_TOKEN.is_match(trimmed)
}

// ======================================================================
// 公開用スニペット
// ======================================================================
/// 通知やメニュー表示用にテキストを切り詰め、機密らしければマスクする
///
/// # Arguments
/// * `text` - 表示対象テキスト
/// * `max_chars` - 最大文字数 (`...` 含む)
pub fn format_public_snippet(text: &str, max_chars: usize) -> String {
    if looks_sensitive(text) {
        return SENSITIVE_SNIPPET_LABEL.to_string();
    }

    truncate_chars(text, max_chars)
}

/// 文字数で切り詰める (`max_chars` 超過時は末尾に `...`)
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    format!("{}...", text.chars().take(keep).collect::<String>())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 通常テキストは機密と判定しないこと
    #[test]
    fn looks_sensitive_allows_plain_text() {
        assert!(!looks_sensitive("hello world"));
        assert!(!looks_sensitive("https://example.com/path?q=1"));
    }

    /// PEM 秘密鍵を検出すること
    #[test]
    fn looks_sensitive_detects_pem() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----";
        assert!(looks_sensitive(pem));
    }

    /// JWT を検出すること
    #[test]
    fn looks_sensitive_detects_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        assert!(looks_sensitive(jwt));
    }

    /// 資格情報行を検出すること
    #[test]
    fn looks_sensitive_detects_credential_line() {
        assert!(looks_sensitive("api_key=sk_live_abcdefghijklmnop"));
        assert!(looks_sensitive("PASSWORD: hunter2"));
    }

    /// 裸の長いトークンを検出すること
    #[test]
    fn looks_sensitive_detects_bare_token() {
        assert!(looks_sensitive(
            "dGhpcyBpcyBhIGxvbmcgc2VjcmV0IHRva2VuX3ZhbHVl"
        ));
    }

    /// 機密テキストはプレースホルダーに置換すること
    #[test]
    fn format_public_snippet_masks_sensitive() {
        let masked = format_public_snippet("api_key=supersecret", 50);
        assert_eq!(masked, SENSITIVE_SNIPPET_LABEL);
    }

    /// 通常テキストは切り詰めること
    #[test]
    fn format_public_snippet_truncates() {
        let input = "あ".repeat(51);
        let snippet = format_public_snippet(&input, 50);
        assert!(snippet.ends_with("..."));
        assert_eq!(snippet.chars().count(), 50);
    }
}
