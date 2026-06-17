use std::borrow::Cow;

use crate::refiner::OrderedValue;

use serde_norway::Value;

// ======================================================================
// YAML → JSON 変換
// ======================================================================
/// YAML文字列をJSON文字列へ変換する(キー順序不同)
///
/// 入力されたYAMLを解析し、対応するJSON形式の文字列に変換する
/// キーの順序は保証されない
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のJSON文字列。パースに失敗した場合は元の文字列を返す。
pub fn yaml_to_json(text: &str) -> Cow<'_, str> {
    let v: Value = match serde_norway::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            crate::log_debug!("YAML パースに失敗 (yaml_to_json): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json_text) => Cow::Owned(json_text),
        Err(e) => {
            crate::log_debug!("JSON 変換に失敗 (yaml_to_json): {}", e);
            Cow::Borrowed(text)
        }
    }
}

/// YAML文字列をJSON文字列へ変換する(キー順序保持)
///
/// 入力されたYAMLを解析し、対応するJSON形式の文字列に変換する
/// 可能な限り、元のYAMLにおけるマップのキー順序を維持して変換を試みる
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のJSON文字列。パースに失敗した場合は元の文字列を返す。
pub fn yaml_to_json_preserve_order(text: &str) -> Cow<'_, str> {
    let v: OrderedValue = match serde_norway::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            crate::log_debug!("YAML パースに失敗 (yaml_to_json_preserve_order): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json) => Cow::Owned(json),
        Err(e) => {
            crate::log_debug!("JSON 変換に失敗 (yaml_to_json_preserve_order): {}", e);
            Cow::Borrowed(text)
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 有効な YAML を JSON に変換できること
    #[test]
    fn test_yaml_to_json_valid() {
        let input = "b: 1\na: 2\n";
        let output = yaml_to_json(input);
        assert!(output.contains("\"a\": 2"));
    }

    /// 不正な YAML は元の文字列を返すこと
    #[test]
    fn test_yaml_to_json_invalid() {
        let input = "a: 1\n  b: 2";
        assert_eq!(yaml_to_json(input), input);
    }

    /// 有効な YAML をキー順序を維持した JSON に変換できること
    #[test]
    fn test_yaml_to_json_preserve_order_valid() {
        let input = "b: 1\na: 2\n";
        let output = yaml_to_json_preserve_order(input);
        let b_pos = output.find("\"b\"").expect("b キーが含まれる");
        let a_pos = output.find("\"a\"").expect("a キーが含まれる");
        assert!(b_pos < a_pos, "YAML のキー順序が維持されていない");
    }

    /// 不正な YAML (キー順序保持版) は元の文字列を返すこと
    #[test]
    fn test_yaml_to_json_preserve_order_invalid() {
        let input = "a: 1\n  b: 2";
        assert_eq!(yaml_to_json_preserve_order(input), input);
    }
}
