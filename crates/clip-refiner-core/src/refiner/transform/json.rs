use std::borrow::Cow;

use crate::refiner::OrderedValue;
use crate::security::is_within_parser_limit;

use serde::ser::{Error as _, Serialize, Serializer};
use serde_json::{Number, Value};

// ======================================================================
// JSON 整形
// ======================================================================
/// JSON文字列を整形(Pretty Print)する(キー順序ソート)
///
/// 入力されたJSON文字列を解析し、インデントを追加して読みやすい形式に整形する
/// オブジェクトのキーは辞書順に並べ替える
///
/// # Arguments
/// * `text` - 整形対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 整形済みのJSON文字列。パースに失敗した場合は元の文字列を返す。
pub fn format_json(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (format_json)");
        return Cow::Borrowed(text);
    }

    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("JSON パースに失敗 (format_json): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => {
            if pretty == text {
                Cow::Borrowed(text)
            } else {
                Cow::Owned(pretty)
            }
        }
        Err(e) => {
            tracing::debug!("JSON シリアライズに失敗 (format_json): {}", e);
            Cow::Borrowed(text)
        }
    }
}

/// JSON文字列を整形(Pretty Print)する(キー順序保持)
///
/// 入力されたJSON文字列を解析し、インデントを追加して読みやすい形式に整形する
/// 元のJSONに含まれるオブジェクトのキーの順序を可能な限り維持する
///
/// # Arguments
/// * `text` - 整形対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 整形済みのJSON文字列。パースに失敗した場合は元の文字列を返す。
pub fn format_json_preserve_order(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (format_json_preserve_order)");
        return Cow::Borrowed(text);
    }

    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("JSON パースに失敗 (format_json_preserve_order): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => {
            if pretty == text {
                Cow::Borrowed(text)
            } else {
                Cow::Owned(pretty)
            }
        }
        Err(e) => {
            tracing::debug!(
                "JSON シリアライズに失敗 (format_json_preserve_order): {}",
                e
            );
            Cow::Borrowed(text)
        }
    }
}

// ======================================================================
// JSON → YAML 変換
// ======================================================================
/// JSON文字列をYAML文字列へ変換する(キー順序ソート)
///
/// 入力されたJSONを解析し、対応するYAML形式の文字列に変換する
/// オブジェクトのキーは辞書順に並べ替える
///
/// # Arguments
/// * `text` - 変換対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のYAML文字列。パースに失敗した場合は元の文字列を返す。
pub fn json_to_yaml(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (json_to_yaml)");
        return Cow::Borrowed(text);
    }

    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("JSON パースに失敗 (json_to_yaml): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_norway::to_string(&YamlValue(&v)) {
        Ok(yaml_text) => Cow::Owned(yaml_text),
        Err(e) => {
            tracing::debug!("YAML 変換に失敗 (json_to_yaml): {}", e);
            Cow::Borrowed(text)
        }
    }
}

/// JSON文字列をYAML文字列へ変換する(キー順序保持)
///
/// 入力されたJSONを解析し、対応するYAML形式の文字列に変換する
/// 元のJSONに含まれるオブジェクトのキーの順序を可能な限り維持する
///
/// # Arguments
/// * `text` - 変換対象のJSON文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のYAML文字列。パースに失敗した場合は元の文字列を返す。
pub fn json_to_yaml_preserve_order(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (json_to_yaml_preserve_order)");
        return Cow::Borrowed(text);
    }

    let v: OrderedValue = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("JSON パースに失敗 (json_to_yaml_preserve_order): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_norway::to_string(&YamlOrderedValue(&v)) {
        Ok(yaml_text) => Cow::Owned(yaml_text),
        Err(e) => {
            tracing::debug!("YAML 変換に失敗 (json_to_yaml_preserve_order): {}", e);
            Cow::Borrowed(text)
        }
    }
}

// ======================================================================
// YAML 出力用の数値変換
// ======================================================================
/// `serde_json::Value` を YAML の数値表現で直列化するラッパー
///
/// `arbitrary_precision` 有効時の `Number` は JSON 以外の直列化先で内部構造体として
/// 出力されるため、数値だけ `serialize_yaml_number` で整数・浮動小数点へ変換する
struct YamlValue<'a>(&'a Value);

impl Serialize for YamlValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Number(n) => serialize_yaml_number(n, serializer),
            Value::Array(items) => serializer.collect_seq(items.iter().map(YamlValue)),
            Value::Object(map) => {
                serializer.collect_map(map.iter().map(|(k, v)| (k, YamlValue(v))))
            }
            other => other.serialize(serializer),
        }
    }
}

/// `OrderedValue` を YAML の数値表現で直列化するラッパー
///
/// 数値の扱いは `YamlValue` と同じ
struct YamlOrderedValue<'a>(&'a OrderedValue);

impl Serialize for YamlOrderedValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            OrderedValue::Number(n) => serialize_yaml_number(n, serializer),
            OrderedValue::Array(items) => {
                serializer.collect_seq(items.iter().map(YamlOrderedValue))
            }
            OrderedValue::Object(map) => {
                serializer.collect_map(map.iter().map(|(k, v)| (k, YamlOrderedValue(v))))
            }
            other => other.serialize(serializer),
        }
    }
}

/// JSON の数値を YAML の整数・浮動小数点として直列化する
///
/// 整数は `u128` / `i128` の範囲まで桁を保つ。それ以外は `f64` に丸め、
/// 有限値にならない場合は変換失敗とする
///
/// # Arguments
/// * `n` - 直列化する数値
/// * `serializer` - 直列化先
///
/// # Returns
/// * `Result<S::Ok, S::Error>` - 直列化結果
fn serialize_yaml_number<S: Serializer>(n: &Number, serializer: S) -> Result<S::Ok, S::Error> {
    let text = n.as_str();
    if let Ok(v) = text.parse::<u128>() {
        return serializer.serialize_u128(v);
    }
    if let Ok(v) = text.parse::<i128>() {
        return serializer.serialize_i128(v);
    }
    match n.as_f64() {
        Some(v) if v.is_finite() => serializer.serialize_f64(v),
        _ => Err(S::Error::custom(format!("YAML で表現できない数値: {text}"))),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 有効な JSON を整形できるようにすること
    #[test]
    fn test_format_json_valid() {
        let input = r#"{"b":1,"a":2}"#;
        let output = format_json(input);
        assert!(output.contains("\"a\": 2"));
    }

    /// 不正な JSON は元の文字列を返すようにすること
    #[test]
    fn test_format_json_invalid() {
        let input = r#"{"a":1,"b":}"#;
        assert_eq!(format_json(input), input);
    }

    /// キー順序保持の整形: 元の順序が維持されるようにすること
    #[test]
    fn test_format_json_preserve_order() {
        let input = r#"{"z":1,"a":2,"m":3}"#;
        let output = format_json_preserve_order(input);
        let z_pos = output.find("\"z\"").unwrap();
        let a_pos = output.find("\"a\"").unwrap();
        let m_pos = output.find("\"m\"").unwrap();
        assert!(z_pos < a_pos && a_pos < m_pos, "キー順序が保持されていない");
    }

    /// キー順序ソートの整形: キーが辞書順になるようにすること
    #[test]
    fn test_format_json_unordered_sorts_keys() {
        let input = r#"{"z":1,"a":2}"#;
        let output = format_json(input);
        let a_pos = output.find("\"a\"").unwrap();
        let z_pos = output.find("\"z\"").unwrap();
        assert!(a_pos < z_pos, "キーが辞書順に並んでいない");
    }

    /// 不正 JSON は `preserve_order` でも元の文字列を返すようにすること
    #[test]
    fn test_format_json_preserve_order_invalid() {
        let input = r#"{"a":1,}"#;
        assert_eq!(format_json_preserve_order(input), input);
    }

    /// `json_to_yaml`: JSONをYAMLに変換できるようにすること
    #[test]
    fn test_json_to_yaml_valid() {
        let input = r#"{"name":"Alice","age":30}"#;
        let output = json_to_yaml(input);
        assert!(output.contains("name: Alice"));
        assert!(output.contains("age: 30"));
    }

    /// `json_to_yaml`: 不正 JSON は元の文字列を返すようにすること
    #[test]
    fn test_json_to_yaml_invalid() {
        let input = r#"{"a":}"#;
        assert_eq!(json_to_yaml(input), input);
    }

    /// `json_to_yaml_preserve_order`: キー順序が維持されるようにすること
    #[test]
    fn test_json_to_yaml_preserve_order() {
        let input = r#"{"z":1,"a":2,"m":3}"#;
        let output = json_to_yaml_preserve_order(input);
        let z_pos = output.find("z:").unwrap();
        let a_pos = output.find("a:").unwrap();
        let m_pos = output.find("m:").unwrap();
        assert!(z_pos < a_pos && a_pos < m_pos, "キー順序が保持されていない");
    }

    /// `json_to_yaml`: キーが辞書順になるようにすること
    #[test]
    fn test_json_to_yaml_sorts_keys() {
        let input = r#"{"z":1,"a":2}"#;
        let output = json_to_yaml(input);
        let a_pos = output.find("a:").unwrap();
        let z_pos = output.find("z:").unwrap();
        assert!(a_pos < z_pos, "キーが辞書順に並んでいない");
    }

    /// 配列を含む JSON の変換できるようにすること
    #[test]
    fn test_json_to_yaml_array() {
        let input = r"[1,2,3]";
        let output = json_to_yaml(input);
        assert!(output.contains("- 1"));
        assert!(output.contains("- 2"));
        assert!(output.contains("- 3"));
    }

    /// 既に Pretty な JSON は借用のまま返すようにすること
    #[test]
    fn test_format_json_already_pretty_returns_borrowed() {
        let input = "{\n  \"a\": 1\n}";
        let output = format_json(input);
        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(output.as_ref(), input);
    }

    /// パーサ上限超過の入力は整形をスキップすること
    #[test]
    fn test_format_json_skips_oversized_input() {
        use crate::consts::MAX_PARSER_INPUT_BYTES;

        let input = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
        let output = format_json(&input);
        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(output.len(), input.len());
    }

    /// パーサ上限超過の入力は YAML 変換をスキップすること
    #[test]
    fn test_json_to_yaml_skips_oversized_input() {
        use crate::consts::MAX_PARSER_INPUT_BYTES;

        let input = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
        let output = json_to_yaml(&input);
        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(output.len(), input.len());
    }

    /// 倍精度を超える整数を丸めずに整形すること
    #[test]
    fn test_format_json_keeps_large_integer() {
        let input = r#"{"z":12345678901234567890123,"a":-98765432109876543210}"#;
        // キー順序ソート・保持のどちらでも元の桁を保つ
        for output in [format_json(input), format_json_preserve_order(input)] {
            assert!(output.contains("12345678901234567890123"), "{output}");
            assert!(output.contains("-98765432109876543210"), "{output}");
        }
    }

    /// 小数の表記を元の JSON のまま整形すること
    #[test]
    fn test_format_json_keeps_decimal_text() {
        let input = r"[1.10,0.1000000000000000055511151231257827,1e400]";
        let output = format_json(input);
        assert!(output.contains("1.10"), "{output}");
        assert!(
            output.contains("0.1000000000000000055511151231257827"),
            "{output}"
        );
        // 指数表記は符号付きに正規化される
        assert!(output.contains("1e+400"), "{output}");
    }

    /// YAML 変換で倍精度を超える整数を丸めずに数値として出力すること
    #[test]
    fn test_json_to_yaml_keeps_large_integer() {
        let input = r#"{"a":12345678901234567890123,"b":-98765432109876543210,"c":1.5}"#;
        // キー順序ソート・保持のどちらでも内部表現を出さず数値として出力する
        for output in [json_to_yaml(input), json_to_yaml_preserve_order(input)] {
            assert!(
                output.contains(
                    "a: 12345678901234567890123
"
                ),
                "{output}"
            );
            assert!(
                output.contains(
                    "b: -98765432109876543210
"
                ),
                "{output}"
            );
            assert!(
                output.contains(
                    "c: 1.5
"
                ),
                "{output}"
            );
            assert!(!output.contains("serde_json"), "{output}");
        }
    }

    /// YAML で表現できない数値は元の文字列を返すこと
    #[test]
    fn test_json_to_yaml_out_of_range_number_returns_input() {
        let input = r#"{"a":1e400}"#;
        assert_eq!(json_to_yaml(input), input);
        assert_eq!(json_to_yaml_preserve_order(input), input);
    }
}
