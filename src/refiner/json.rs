use std::borrow::Cow;

use crate::refiner::OrderedValue;

use serde_json::Value;

// ======================================================================
// JSON 整形
// ======================================================================
/// JSON文字列を整形(Pretty Print)する(キー順序不同)
///
/// 入力されたJSON文字列を解析し、インデントを追加して読みやすい形式に整形します。
/// キーの順序は保証されません。
///
/// # Arguments
/// * `text` - 整形対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 整形済みのJSON文字列。パースに失敗した場合は元の文字列を返します。
pub fn format_json(text: &str) -> Cow<'_, str> {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => {
            if pretty == text {
                Cow::Borrowed(text)
            } else {
                Cow::Owned(pretty)
            }
        }
        Err(_) => Cow::Borrowed(text),
    }
}

/// JSON文字列を整形(Pretty Print)する(キー順序保持)
///
/// 入力されたJSON文字列を解析し、インデントを追加して読みやすい形式に整形します。
/// 元のJSONに含まれるオブジェクトのキーの順序を可能な限り維持します。
///
/// # Arguments
/// * `text` - 整形対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 整形済みのJSON文字列。パースに失敗した場合は元の文字列を返します。
pub fn format_json_preserve_order(text: &str) -> Cow<'_, str> {
    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => {
            if pretty == text {
                Cow::Borrowed(text)
            } else {
                Cow::Owned(pretty)
            }
        }
        Err(_) => Cow::Borrowed(text),
    }
}

// ======================================================================
// JSON → YAML 変換
// ======================================================================
/// JSON文字列をYAML文字列へ変換する(キー順序不同)
///
/// 入力されたJSONを解析し、対応するYAML形式の文字列に変換します。
/// キーの順序は保証されません。
///
/// # Arguments
/// * `text` - 変換対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のYAML文字列。パースに失敗した場合は元の文字列を返します。
pub fn json_to_yaml(text: &str) -> Cow<'_, str> {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_yml::to_string(&v) {
        Ok(yaml_text) => Cow::Owned(yaml_text),
        Err(_) => Cow::Borrowed(text),
    }
}

/// JSON文字列をYAML文字列へ変換する(キー順序保持)
///
/// 入力されたJSONを解析し、対応するYAML形式の文字列に変換します。
/// 元のJSONに含まれるオブジェクトのキーの順序を可能な限り維持します。
///
/// # Arguments
/// * `text` - 変換対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のYAML文字列。パースに失敗した場合は元の文字列を返します。
pub fn json_to_yaml_preserve_order(text: &str) -> Cow<'_, str> {
    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_yml::to_string(&v) {
        Ok(yaml_text) => Cow::Owned(yaml_text),
        Err(_) => Cow::Borrowed(text),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_json_valid() {
        let input = r#"{"b":1,"a":2}"#;
        let output = format_json(input);
        assert!(output.contains("\"a\": 2"));
    }

    #[test]
    fn test_format_json_invalid() {
        let input = r#"{"a":1,"b":}"#;
        assert_eq!(format_json(input), input);
    }

    /// キー順序保持の整形: 元の順序が維持されること
    #[test]
    fn test_format_json_preserve_order() {
        let input = r#"{"z":1,"a":2,"m":3}"#;
        let output = format_json_preserve_order(input);
        let z_pos = output.find("\"z\"").unwrap();
        let a_pos = output.find("\"a\"").unwrap();
        let m_pos = output.find("\"m\"").unwrap();
        assert!(z_pos < a_pos && a_pos < m_pos, "キー順序が保持されていない");
    }

    /// キー順序不同の整形: キーがアルファベット順(またはソート済み)になること
    #[test]
    fn test_format_json_unordered_sorts_keys() {
        let input = r#"{"z":1,"a":2}"#;
        let output = format_json(input);
        let a_pos = output.find("\"a\"").unwrap();
        let z_pos = output.find("\"z\"").unwrap();
        assert!(a_pos < z_pos, "serde_json はキーをソートするはず");
    }

    /// 不正 JSON は preserve_order でも元の文字列を返すこと
    #[test]
    fn test_format_json_preserve_order_invalid() {
        let input = r#"{"a":1,}"#;
        assert_eq!(format_json_preserve_order(input), input);
    }

    /// json_to_yaml: JSONをYAMLに変換できること
    #[test]
    fn test_json_to_yaml_valid() {
        let input = r#"{"name":"Alice","age":30}"#;
        let output = json_to_yaml(input);
        assert!(output.contains("name: Alice"));
        assert!(output.contains("age: 30"));
    }

    /// json_to_yaml: 不正 JSON は元の文字列を返すこと
    #[test]
    fn test_json_to_yaml_invalid() {
        let input = r#"{"a":}"#;
        assert_eq!(json_to_yaml(input), input);
    }

    /// json_to_yaml_preserve_order: キー順序が維持されること
    #[test]
    fn test_json_to_yaml_preserve_order() {
        let input = r#"{"z":1,"a":2,"m":3}"#;
        let output = json_to_yaml_preserve_order(input);
        let z_pos = output.find("z:").unwrap();
        let a_pos = output.find("a:").unwrap();
        let m_pos = output.find("m:").unwrap();
        assert!(z_pos < a_pos && a_pos < m_pos, "キー順序が保持されていない");
    }

    /// 配列を含む JSON の変換
    #[test]
    fn test_json_to_yaml_array() {
        let input = r#"[1,2,3]"#;
        let output = json_to_yaml(input);
        assert!(output.contains("- 1"));
        assert!(output.contains("- 2"));
        assert!(output.contains("- 3"));
    }
}
