//! 設定 / セレクター `WebView` の CSP と遷移許可判定
//!
//! 表示内容は埋め込み HTML (`NavigateToString`) のみ。外部 URL の読み込みや
//! ページ遷移は許可しない

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

// ======================================================================
// 定数
// ======================================================================
/// CSP 本文のうち `script-src` を除く部分 (インライン style と `data:` 画像のみ許可)
///
/// CSS は HTML へインライン展開する。アプリアイコンと登録クリップサムネイルは
/// `data:` URL。`connect-src` 等は `default-src 'none'` で禁止する
const WEBVIEW_CSP_BASE: &str = concat!(
    "default-src 'none'; ",
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
/// 埋め込み JS を囲むインライン `<script>` ブロックを組み立てる
///
/// `apply_webview_csp` が許可するハッシュと一致する本文になる。JS を HTML へ埋め込むときは
/// 必ずこの関数を使う
///
/// # Arguments
/// * `js` - 埋め込むバンドル JS
///
/// # Returns
/// * `String` - `<script>` から `</script>` までのブロック
pub(crate) fn inline_script_block(js: &str) -> String {
    format!("<script>{}</script>", inline_script_body(js))
}

/// HTML の CSP プレースホルダを本番ポリシーへ置換する
///
/// `script-src` は `inline_script_block(js)` で埋め込んだ固定スクリプトのハッシュのみ許可する。
/// ハッシュは HTML を走査せず `js` から計算するため、後から HTML に混入した
/// `<script>` は実行されない
///
/// # Arguments
/// * `html` - `__WEBVIEW_CSP__` を含む HTML
/// * `js` - `inline_script_block` で埋め込んだ JS
///
/// # Returns
/// * `String` - CSP 本文を埋め込んだ HTML
pub(crate) fn apply_webview_csp(html: &str, js: &str) -> String {
    let script_hash = script_hash_source(&inline_script_body(js));
    let csp = format!("script-src {script_hash}; {WEBVIEW_CSP_BASE}");
    html.replace(WEBVIEW_CSP_PLACEHOLDER, &csp)
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
/// インライン `<script>` の本文を返す (CSP ハッシュの計算対象)
///
/// HTML パーサは改行を LF に正規化してから本文を扱うため、CRLF / CR は LF にそろえる
/// (ソースが CRLF で checkout されてもハッシュが一致する)
fn inline_script_body(js: &str) -> String {
    let normalized = js.replace("\r\n", "\n").replace('\r', "\n");
    format!("\n{normalized}\n")
}

/// スクリプト本文の CSP ソース式 (`'sha256-<base64>'`) を返す
fn script_hash_source(body: &str) -> String {
    format!(
        "'sha256-{}'",
        STANDARD.encode(Sha256::digest(body.as_bytes()))
    )
}

/// 埋め込み済み HTML のインラインスクリプトがすべて CSP で許可されていることを検証する
///
/// `html` の CSP メタタグに含まれるハッシュと、属性なし `<script>` ブロックの本文を突き合わせる
///
/// # Arguments
/// * `html` - 組み立て済みの HTML
#[cfg(test)]
pub(crate) fn assert_inline_scripts_allowed_by_csp(html: &str) {
    let marker = r#"http-equiv="Content-Security-Policy" content=""#;
    let start = html.find(marker).expect("CSP メタタグ") + marker.len();
    let csp = &html[start..start + html[start..].find('"').expect("CSP 終端")];
    assert!(
        !csp.contains("script-src 'unsafe-inline'"),
        "script-src にインライン全許可を含まないこと: {csp}"
    );

    let mut rest = html;
    let mut count = 0;
    while let Some(open) = rest.find("<script>") {
        let body_start = open + "<script>".len();
        let body_len = rest[body_start..].find("</script>").expect("</script>");
        let body = &rest[body_start..body_start + body_len];
        let hash = script_hash_source(body);
        assert!(
            csp.contains(&hash),
            "スクリプトのハッシュ {hash} を許可すること"
        );
        count += 1;
        rest = &rest[body_start + body_len..];
    }
    assert!(count > 0, "インラインスクリプトが 1 つ以上あること");
}

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

    /// CSP プレースホルダをポリシー本文へ置換し、スクリプトをハッシュで許可すること
    #[test]
    fn apply_webview_csp_replaces_placeholder() {
        let html = apply_webview_csp(
            r#"<meta http-equiv="Content-Security-Policy" content="__WEBVIEW_CSP__">"#,
            "console.log(1);",
        );
        assert!(!html.contains(WEBVIEW_CSP_PLACEHOLDER));
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains("img-src data:"));
        assert!(html.contains("script-src 'sha256-"));
        // 任意のインラインスクリプトを許可しない
        assert!(!html.contains("script-src 'unsafe-inline'"));
    }

    /// 既知のスクリプト本文に対して CSP ハッシュが期待値と一致すること
    ///
    /// 期待値は `printf '\nconsole.log(1);\n' | openssl dgst -sha256 -binary | base64` の結果
    #[test]
    fn script_hash_source_matches_known_digest() {
        let body = inline_script_body("console.log(1);");
        assert_eq!(body, "\nconsole.log(1);\n");
        assert_eq!(
            script_hash_source(&body),
            "'sha256-6ylx2nk61t31O3rHPzQZfJJkryyz5Ldd+rHtiDdohm0='"
        );
    }

    /// CRLF の JS でも HTML 上の本文と CSP ハッシュが一致すること
    #[test]
    fn inline_script_block_normalizes_newlines() {
        let lf = inline_script_block("a();\nb();");
        let crlf = inline_script_block("a();\r\nb();");
        let cr = inline_script_block("a();\rb();");
        assert_eq!(lf, crlf);
        assert_eq!(lf, cr);
        assert!(!lf.contains('\r'));
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
