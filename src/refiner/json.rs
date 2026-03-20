use std::borrow::Cow;

use crate::refiner::OrderedValue;

use serde_json::Value;

/// JSON文字列を整形(Pretty Print)する(キー順序不同)
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
pub fn json_to_yaml(text: &str) -> Cow<'_, str> {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_yaml::to_string(&v) {
        Ok(yaml_text) => Cow::Owned(yaml_text),
        Err(_) => Cow::Borrowed(text),
    }
}

/// JSON文字列をYAML文字列へ変換する(キー順序保持)
pub fn json_to_yaml_preserve_order(text: &str) -> Cow<'_, str> {
    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Cow::Borrowed(text),
    };

    match serde_yaml::to_string(&v) {
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
