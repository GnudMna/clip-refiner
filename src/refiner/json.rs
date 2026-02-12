use crate::refiner::OrderedValue;

use serde_json::Value;
use serde_yaml;

/// JSON文字列を整形(Pretty Print)する(キー順序不同)
/// 整形に失敗した(有効なJSONではない)場合は元の文字列を返す
///
/// # Arguments
/// * `text` - 整形するJSON文字列。
///
/// # Returns
/// * `String` - 整形されたJSON文字列。パースに失敗した場合は元の文字列を返す。
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

/// JSON文字列を整形(Pretty Print)する(キー順序保持)
/// 整形に失敗した(有効なJSONではない)場合は元の文字列を返す
///
/// # Arguments
/// * `text` - 整形するJSON文字列。
///
/// # Returns
/// * `String` - キー順序を保持して整形されたJSON文字列。パースに失敗した場合は元の文字列を返す。
pub fn format_json_preserve_order(text: &str) -> String {
    // JSON文字列をrefiner::OrderedValueへパース
    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // 整形
    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => pretty,
        Err(_) => text.to_string(),
    }
}

/// JSON文字列をYAML文字列へ変換する(キー順序不同)
/// 整形に失敗した(有効なJSONではない)場合は元の文字列を返す
///
/// # Arguments
/// * `text` - 変換するJSON文字列。
///
/// # Returns
/// * `String` - 変換されたYAML文字列。パースに失敗した場合は元の文字列を返す。
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

/// JSON文字列をYAML文字列へ変換する(キー順序保持)
/// 整形に失敗した(有効なJSONではない)場合は元の文字列を返す
///
/// # Arguments
/// * `text` - 変換するJSON文字列。
///
/// # Returns
/// * `String` - キー順序を保持して変換されたYAML文字列。パースに失敗した場合は元の文字列を返す。
pub fn json_to_yaml_preserve_order(text: &str) -> String {
    // JSON文字列をrefiner::OrderedValueへパース
    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };

    // serde_yamlでYAML文字列へ変換
    match serde_yaml::to_string(&v) {
        Ok(yaml_text) => yaml_text,
        Err(_) => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 有効なJSONの整形テスト
    #[test]
    fn test_format_json_valid() {
        let input = r#"{"b":1,"a":2}"#;
        let output = format_json(input);

        // serde_json::Value はキー順序を保持しないため、整形後は順序が変わる可能性がある
        let expected_v: Value = serde_json::from_str(input).unwrap();
        let expected = serde_json::to_string_pretty(&expected_v).unwrap();

        assert_eq!(output, expected);
    }

    /// 無効なJSONの整形テスト
    /// 元の文字列がそのまま返ることを確認
    #[test]
    fn test_format_json_invalid() {
        let input = r#"{"a":1,"b":}"#;
        let output = format_json(input);
        assert_eq!(output, input);
    }

    /// 有効なJSONの整形（キー順序保持）テスト
    #[test]
    fn test_format_json_preserve_order_valid() {
        let input = r#"{"z":1,"a":2,"m":3}"#;
        let output = format_json_preserve_order(input);

        // OrderedValue がキー順序を保持して整形されることを期待
        let expected = r#"{
  "z": 1,
  "a": 2,
  "m": 3
}"#;

        assert_eq!(output, expected);
    }

    /// 無効なJSONの整形（キー順序保持）テスト
    #[test]
    fn test_format_json_preserve_order_invalid() {
        let input = r#"{"x":1,"y":}"#; // invalid JSON
        let output = format_json_preserve_order(input);
        assert_eq!(output, input);
    }

    /// 有効なJSONからYAMLへの変換テスト
    #[test]
    fn test_json_to_yaml_valid() {
        let input = r#"{"b":1,"a":2}"#;
        let output = json_to_yaml(input);

        // serde_json::Value は順序を保持しないため、YAML のキー順序は保証されない
        let v: Value = serde_json::from_str(input).unwrap();
        let expected = serde_yaml::to_string(&v).unwrap();

        assert_eq!(output, expected);
    }

    /// 無効なJSONからYAMLへの変換テスト
    #[test]
    fn test_json_to_yaml_invalid() {
        let input = r#"{"a":1,"b":}"#;
        let output = json_to_yaml(input);
        assert_eq!(output, input);
    }

    /// 有効なJSONからYAMLへの変換（キー順序保持）テスト
    #[test]
    fn test_json_to_yaml_preserve_order_valid() {
        let input = r#"{"z":1,"a":2}"#;
        let output = json_to_yaml_preserve_order(input);

        // OrderedValue により順序保持されることを期待
        let expected = "z: 1\na: 2\n";

        assert_eq!(output, expected);
    }

    /// 無効なJSONからYAMLへの変換（キー順序保持）テスト
    #[test]
    fn test_json_to_yaml_preserve_order_invalid() {
        let input = r#"{"x":1,"y":}"#;
        let output = json_to_yaml_preserve_order(input);
        assert_eq!(output, input);
    }

    /// 空のJSONオブジェクト・配列の整形テスト
    #[test]
    fn test_format_json_empty() {
        assert_eq!(format_json("{}"), "{}");
        assert_eq!(format_json("[]"), "[]");
    }

    /// ネストされたJSONの整形テスト
    #[test]
    fn test_format_json_nested() {
        let input = r#"{"a":{"b":1}}"#;
        let output = format_json(input);
        assert!(output.contains(r#""b": 1"#));
    }
}
