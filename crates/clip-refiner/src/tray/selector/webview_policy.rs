//! 設定 / セレクター `WebView` の CSP と遷移許可判定
//!
//! 表示内容は埋め込み HTML (`NavigateToString`) のみ。外部 URL の読み込みや
//! ページ遷移は許可しない

use base64::{Engine as _, engine::general_purpose::STANDARD};

// ======================================================================
// 定数
// ======================================================================
/// HTML に埋め込む CSP 本文 (インライン script / style と `data:` 画像のみ)
///
/// バンドル JS と CSS は HTML へインライン展開する。アプリアイコンと登録クリップ
/// サムネイルは `data:` URL。`connect-src` 等は `default-src 'none'` で禁止する
pub(crate) const WEBVIEW_CSP: &str = concat!(
    "default-src 'none'; ",
    "script-src 'unsafe-inline'; ",
    "style-src 'unsafe-inline'; ",
    "img-src data:; ",
    "connect-src 'none'; ",
    "frame-src 'none'; ",
    "object-src 'none'; ",
    "base-uri 'none'; ",
    "form-action 'none'",
);

/// HTML ソースの CSP プレースホルダ (`apply_webview_csp` で置換する)
pub(crate) const WEBVIEW_CSP_PLACEHOLDER: &str = "__WEBVIEW_CSP__";

/// 拒否ログに出す URL の最大文字数
const DENIED_URL_PREVIEW_MAX_LEN: usize = 200;

/// UTF-8 BOM
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

// ======================================================================
// パブリック関数
// ======================================================================
/// HTML の CSP プレースホルダを本番ポリシーへ置換する
///
/// # Arguments
/// * `html` - `__WEBVIEW_CSP__` を含む組み立て前 HTML
///
/// # Returns
/// * `String` - CSP 本文を埋め込んだ HTML
pub(crate) fn apply_webview_csp(html: &str) -> String {
    html.replace(WEBVIEW_CSP_PLACEHOLDER, WEBVIEW_CSP)
}

/// `WebView2` のページ遷移を許可するかどうか
///
/// `NavigateToString` は環境により `about:blank` または
/// `data:text/html;charset=utf-8;base64,...` (埋め込んだ HTML そのもの) として発火する
/// 後者は `expected_html` と一致する場合のみ許可し、別 HTML の `data:` 遷移は拒否する
///
/// # Arguments
/// * `url` - `NavigationStarting` で渡される URI
/// * `expected_html` - `with_html` に渡した埋め込み HTML
///
/// # Returns
/// * `bool` - 埋め込み HTML の初期読み込みなら `true`
pub(crate) fn is_allowed_webview_navigation(url: &str, expected_html: &str) -> bool {
    let url = url.trim();
    is_about_blank_navigation(url) || is_embedded_html_data_url(url, expected_html)
}

/// 拒否ログ向けに URL を切り詰める
///
/// # Arguments
/// * `url` - 拒否した遷移先
///
/// # Returns
/// * `String` - 長すぎる場合は先頭 200 文字 + `...`
pub(crate) fn preview_denied_webview_url(url: &str) -> String {
    // バイト位置で切ると多バイト文字の途中で panic するため文字単位で切る
    match url.char_indices().nth(DENIED_URL_PREVIEW_MAX_LEN) {
        Some((end, _)) => format!("{}...", &url[..end]),
        None => url.to_string(),
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// `about:blank` / `about:srcdoc` 系の初期読み込み URI かどうか
fn is_about_blank_navigation(url: &str) -> bool {
    let normalized = url.to_ascii_lowercase();
    normalized == "about:blank"
        || normalized == "about:blank/"
        || normalized == "about:srcdoc"
        || normalized.starts_with("about:blank#")
}

/// `NavigateToString` が `data:text/html;base64` として発火した初期 HTML かどうか
fn is_embedded_html_data_url(url: &str, expected_html: &str) -> bool {
    let Some(payload) = html_data_url_base64_payload(url) else {
        return false;
    };
    let Some(decoded) = decode_base64_payload(payload) else {
        return false;
    };
    let decoded = decoded.strip_prefix(UTF8_BOM).unwrap_or(&decoded);
    decoded == expected_html.as_bytes()
}

/// `data:text/html` かつ `;base64` のペイロードを取り出す
fn html_data_url_base64_payload(url: &str) -> Option<&str> {
    let comma = url.find(',')?;
    let header = url[..comma].to_ascii_lowercase();
    let rest = header.strip_prefix("data:")?;
    let mut parts = rest.split(';');
    if parts.next()? != "text/html" {
        return None;
    }
    if !parts.any(|part| part == "base64") {
        return None;
    }
    Some(&url[comma + 1..])
}

/// base64 ペイロードを復号する (パディング省略にも対応)
fn decode_base64_payload(payload: &str) -> Option<Vec<u8>> {
    let payload = payload.trim();
    if let Ok(bytes) = STANDARD.decode(payload) {
        return Some(bytes);
    }
    let mut padded = payload.to_string();
    while !padded.is_empty() && !padded.len().is_multiple_of(4) {
        padded.push('=');
    }
    STANDARD.decode(padded).ok()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `NavigateToString` 相当の data URL を組み立てる
    fn html_data_url(html: &str) -> String {
        format!(
            "data:text/html;charset=utf-8;base64,{}",
            STANDARD.encode(html.as_bytes())
        )
    }

    /// 埋め込み HTML の初期読み込み URI を許可すること
    #[test]
    fn allows_about_blank_and_matching_html_data_url() {
        let html = "<!DOCTYPE html>\n<html lang=\"ja\"></html>";
        assert!(is_allowed_webview_navigation("about:blank", html));
        assert!(is_allowed_webview_navigation(" about:blank ", html));
        assert!(is_allowed_webview_navigation("ABOUT:BLANK", html));
        assert!(is_allowed_webview_navigation("about:blank/", html));
        assert!(is_allowed_webview_navigation("about:blank#top", html));
        assert!(is_allowed_webview_navigation("about:srcdoc", html));
        assert!(is_allowed_webview_navigation(&html_data_url(html), html));
        let encoded = STANDARD.encode(html.as_bytes());
        let upper_header = format!("DATA:TEXT/HTML;CHARSET=UTF-8;BASE64,{encoded}");
        assert!(is_allowed_webview_navigation(&upper_header, html));
    }

    /// パディング省略の data URL も初期 HTML として許可すること
    #[test]
    fn allows_unpadded_matching_html_data_url() {
        let html = "<!DOCTYPE>";
        let padded = STANDARD.encode(html.as_bytes());
        let unpadded = padded.trim_end_matches('=');
        let url = format!("data:text/html;charset=utf-8;base64,{unpadded}");
        assert!(is_allowed_webview_navigation(&url, html));
    }

    /// 外部・ファイル・別 HTML の data・javascript へのページ遷移を拒否すること
    #[test]
    fn denies_external_and_unrelated_data_navigations() {
        let html = "<!DOCTYPE html>\n<html lang=\"ja\"></html>";
        let other_html = html_data_url("<html>other</html>");
        for url in [
            "https://example.com/",
            "http://127.0.0.1/",
            "file:///C:/Windows/notepad.exe",
            "data:text/html,<script>alert(1)</script>",
            "javascript:alert(1)",
            "ms-appx-web://example",
            "",
            "about:config",
            other_html.as_str(),
            "data:image/png;base64,AAAA",
        ] {
            assert!(
                !is_allowed_webview_navigation(url, html),
                "拒否されること: {url}"
            );
        }
    }

    /// CSP プレースホルダをポリシー本文へ置換すること
    #[test]
    fn apply_webview_csp_replaces_placeholder() {
        let html = apply_webview_csp(
            r#"<meta http-equiv="Content-Security-Policy" content="__WEBVIEW_CSP__">"#,
        );
        assert!(!html.contains(WEBVIEW_CSP_PLACEHOLDER));
        assert!(html.contains(WEBVIEW_CSP));
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains("img-src data:"));
    }

    /// 長い拒否 URL を切り詰めること
    #[test]
    fn preview_denied_webview_url_truncates() {
        let long = "https://example.com/".to_string() + &"a".repeat(300);
        let preview = preview_denied_webview_url(&long);
        assert!(preview.ends_with("..."));
        assert_eq!(preview.len(), DENIED_URL_PREVIEW_MAX_LEN + 3);
        assert_eq!(
            preview_denied_webview_url("https://example.com/"),
            "https://example.com/"
        );
    }

    /// 多バイト文字を含む長い拒否 URL を文字境界で切り詰めること
    #[test]
    fn preview_denied_webview_url_truncates_multibyte() {
        // 先頭 1 バイト + 3 バイト文字の並びで 200 バイト目を文字の途中にする
        let long = "h".to_string() + &"あ".repeat(300);
        let preview = preview_denied_webview_url(&long);
        assert!(preview.ends_with("..."));
        assert_eq!(preview.chars().count(), DENIED_URL_PREVIEW_MAX_LEN + 3);

        let exact = "あ".repeat(DENIED_URL_PREVIEW_MAX_LEN);
        assert_eq!(preview_denied_webview_url(&exact), exact);
    }
}
