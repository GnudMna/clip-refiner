use crate::refiner::OrderedValue;

use serde_json;
use serde_yaml::Value;

/// YAML文字列をJSON文字列へ変換する(キー順序不同)
/// 整形に失敗した(有効なYAMLではない)場合は元の文字列を返す
pub fn yaml_to_json(text: &str) -> String {
    // YAML文字列をserde_yaml::Valueへパース
    let v: Value = match serde_yaml::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // serde_jsonでJSON文字列へ変換(Pretty Print)
    match serde_json::to_string_pretty(&v) {
        Ok(json_text) => json_text,
        Err(_) => text.to_string(),
    }
}

/// YAML文字列をJSON文字列へ変換する(キー順序保持)
/// 整形に失敗した(有効なYAMLではない)場合は元の文字列を返す
pub fn yaml_to_json_preserve_order(text: &str) -> String {
    // YAML文字列をrefiner::OrderedValueへパース
    let v: OrderedValue = match serde_yaml::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // serde_jsonでJSON文字列へ変換(Pretty Print)
    match serde_json::to_string_pretty(&v) {
        Ok(json) => json,
        Err(_) => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    // ---------------------------
    // yaml_to_json
    // ---------------------------
    #[test]
    fn test_yaml_to_json_valid() {
        let input = "b: 1\na: 2\n";
        let output = yaml_to_json(input);

        // serde_yaml::Value はキー順序を保持しないため、JSON のキー順序は保証されない
        let v: Value = serde_yaml::from_str(input).unwrap();
        let expected = serde_json::to_string_pretty(&v).unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_yaml_to_json_invalid() {
        let input = "a: 1\n  b: 2"; // インデント不正
        let output = yaml_to_json(input);
        assert_eq!(output, input);
    }

    // ---------------------------
    // yaml_to_json_preserve_order
    // ---------------------------
    #[test]
    fn test_yaml_to_json_preserve_order_valid() {
        let input = "z: 1\na: 2\nm: 3\n";
        let output = yaml_to_json_preserve_order(input);

        // OrderedValue によりキー順序保持されることを期待
        let expected = r#"{
  "z": 1,
  "a": 2,
  "m": 3
}"#;

        assert_eq!(output, expected);
    }

    #[test]
    fn test_yaml_to_json_preserve_order_invalid() {
        let input = "x: 1\n  y: 2"; // インデント不正
        let output = yaml_to_json_preserve_order(input);
        assert_eq!(output, input);
    }
}
