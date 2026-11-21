//! `WebView` 向け JavaScript 実行ヘルパー

use wry::WebView;

// ======================================================================
// スクリプト生成
// ======================================================================
/// `evaluate_script` 用に JSON 文字列をシングルクォート文字列リテラルへエスケープする
pub(crate) fn escape_json_for_js_single_quoted(json: &str) -> String {
    let mut escaped = String::with_capacity(json.len());
    for ch in json.chars() {
        match ch {
            '\\' => escaped.push_str(r"\\"),
            '\'' => escaped.push_str(r"\'"),
            '\n' => escaped.push_str(r"\n"),
            '\r' => escaped.push_str(r"\r"),
            '\u{2028}' => escaped.push_str(r"\u2028"),
            '\u{2029}' => escaped.push_str(r"\u2029"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// `JSON.parse` 経由で引数を渡す関数呼び出しスクリプトを生成する
pub(crate) fn json_parse_call_script(function_name: &str, json: &str) -> String {
    let json_arg = escape_json_for_js_single_quoted(json);
    format!("{function_name}(JSON.parse('{json_arg}'));")
}

/// 設定スナップショット読み込み用スクリプトを生成する
pub(crate) fn settings_load_script(
    snapshot_json: &str,
    force: bool,
    external_sync: bool,
) -> String {
    let json_arg = escape_json_for_js_single_quoted(snapshot_json);
    let mut opts = Vec::new();
    if force {
        opts.push("force: true");
    }
    if external_sync {
        opts.push("externalSync: true");
    }
    if opts.is_empty() {
        format!("loadSettings(JSON.parse('{json_arg}'));")
    } else {
        format!(
            "loadSettings(JSON.parse('{json_arg}'), {{{}}});",
            opts.join(", ")
        )
    }
}

// ======================================================================
// スクリプト実行
// ======================================================================
/// JavaScript を実行し、失敗時はコンテキスト名付きで警告ログを出す
pub(crate) fn evaluate_script(webview: &WebView, context: &str, script: &str) {
    if let Err(err) = webview.evaluate_script(script) {
        crate::log_warn!("{context} の JavaScript 実行に失敗: {:?}", err);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 設定スナップショット注入が `JSON.parse` 経由であるようにすること
    #[test]
    fn settings_load_script_uses_json_parse() {
        let script = settings_load_script(r#"{"mode":"Trim"}"#, false, false);
        assert!(script.contains("JSON.parse("));
        assert!(!script.contains("loadSettings({"));
    }

    /// 設定値に `'` や `\` が含まれても `JSON.parse` 引数としてエスケープされるようにすること
    #[test]
    fn settings_load_script_escapes_special_characters() {
        let json = r#"{"regex":{"pattern":"it's \\ tricky"}}"#;
        let script = settings_load_script(json, false, false);
        assert!(script.contains(r"it\'s"));
        assert!(!script.contains(r#"pattern":"it's"#));
    }

    /// 改行を含む JSON でもシングルクォート文字列リテラルが壊れないであるようにすること
    #[test]
    fn settings_load_script_escapes_newlines() {
        let json = "{\"regex\":{\"pattern\":\"a\nb\"}}";
        let script = settings_load_script(json, false, false);
        assert!(script.contains(r"a\nb"));
        assert!(!script.contains("parse('{\"regex\":{\"pattern\":\"a\nb"));
    }

    /// 外部同期オプションが JavaScript へ渡されるようにすること
    #[test]
    fn settings_load_script_passes_external_sync_option() {
        let script = settings_load_script(r#"{"mode":"Trim"}"#, false, true);
        assert!(script.contains("externalSync: true"));
    }
}
