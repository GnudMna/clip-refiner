use std::borrow::Cow;

use crate::refiner::OrderedValue;

use serde_yml::Value;

// ======================================================================
// YAML → JSON 変換
// ======================================================================
/// YAML文字列をJSON文字列へ変換する(キー順序不同)
///
/// 入力されたYAMLを解析し、対応するJSON形式の文字列に変換します。
/// キーの順序は保証されません。
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のJSON文字列。パースに失敗した場合は元の文字列を返します。
pub fn yaml_to_json(text: &str) -> Cow<'_, str> {
    let v: Value = match serde_yml::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json_text) => Cow::Owned(json_text),
        Err(_) => Cow::Borrowed(text),
    }
}

/// YAML文字列をJSON文字列へ変換する(キー順序保持)
///
/// 入力されたYAMLを解析し、対応するJSON形式の文字列に変換します。
/// 可能な限り、元のYAMLにおけるマップのキー順序を維持して変換を試みます。
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のJSON文字列。パースに失敗した場合は元の文字列を返します。
pub fn yaml_to_json_preserve_order(text: &str) -> Cow<'_, str> {
    let v: OrderedValue = match serde_yml::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json) => Cow::Owned(json),
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
    fn test_yaml_to_json_valid() {
        let input = "b: 1\na: 2\n";
        let output = yaml_to_json(input);
        assert!(output.contains("\"a\": 2"));
    }

    #[test]
    fn test_yaml_to_json_invalid() {
        let input = "a: 1\n  b: 2";
        assert_eq!(yaml_to_json(input), input);
    }
}
