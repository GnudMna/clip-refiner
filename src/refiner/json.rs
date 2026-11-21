use serde_json::Value;

/// JSON文字列を整形（Pretty Print）する
/// 整形に失敗した（有効なJSONではない）場合は元の文字列を返す
pub fn format_json(text: &str) -> String {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => pretty,
        Err(_) => text.to_string(),
    }
}
