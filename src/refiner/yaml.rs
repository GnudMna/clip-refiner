use serde_json;
use serde_yaml::Value;

/// YAML文字列をJSON文字列へ変換する
/// 整形に失敗した（有効なYAMLではない）場合は元の文字列を返す
pub fn yaml_to_json(text: &str) -> String {
    // YAML文字列をserde_yaml::Valueへパース
    let v: Value = match serde_yaml::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // serde_jsonでJSON文字列へ変換（Pretty Print）
    match serde_json::to_string_pretty(&v) {
        Ok(json_text) => json_text,
        Err(_) => text.to_string(),
    }
}
