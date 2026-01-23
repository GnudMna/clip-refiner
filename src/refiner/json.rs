use serde_json::Value;
use serde_yaml;

/// JSON文字列を整形（Pretty Print）する
/// 整形に失敗した（有効なJSONではない）場合は元の文字列を返す
pub fn format_json(text: &str) -> String {
    // JSON文字列をserde_json::Valueへパース
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // 整形
    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => pretty,
        Err(_) => text.to_string(),
    }
}

/// JSON文字列をYAML文字列へ変換する
/// 整形に失敗した（有効なJSONではない）場合は元の文字列を返す
pub fn json_to_yaml(text: &str) -> String {
    // JSON文字列をserde_json::Valueへパース
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // serde_yamlでYAML文字列へ変換
    match serde_yaml::to_string(&v) {
        Ok(yaml_text) => yaml_text,
        Err(_) => text.to_string(),
    }
}
