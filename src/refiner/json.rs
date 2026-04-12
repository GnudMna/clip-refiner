use std::borrow::Cow;

use crate::refiner::OrderedValue;

use serde_json::Value;

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
}
