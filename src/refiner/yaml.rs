use std::borrow::Cow;

use crate::refiner::OrderedValue;

use serde_yaml::Value;

/// YAML文字列をJSON文字列へ変換する(キー順序不同)
pub fn yaml_to_json(text: &str) -> Cow<'_, str> {
    let v: Value = match serde_yaml::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json_text) => Cow::Owned(json_text),
        Err(_) => Cow::Borrowed(text),
    }
}

/// YAML文字列をJSON文字列へ変換する(キー順序保持)
pub fn yaml_to_json_preserve_order(text: &str) -> Cow<'_, str> {
    let v: OrderedValue = match serde_yaml::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json) => Cow::Owned(json),
        Err(_) => Cow::Borrowed(text),
    }
}

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
