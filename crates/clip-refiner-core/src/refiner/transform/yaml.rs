use std::borrow::Cow;
use std::fmt;

use indexmap::IndexMap;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Number;
use serde_norway::Value;

use crate::refiner::OrderedValue;
use crate::security::is_within_parser_limit;

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
/// * `Cow<'_, str>` - 変換後のJSON文字列。パース・変換に失敗した場合 (`.inf` / `.nan` を含む場合など) や
///   入力がパーサの上限を超える場合は元の文字列を返す
pub fn yaml_to_json(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("YAML 入力が上限を超えているためスキップ (yaml_to_json)");
        return Cow::Borrowed(text);
    }

    // `serde_norway::Value` はタグ付きの値 (`!Ref` など) を扱えるが、`u64` / `i64` を超える整数と
    // 文字列以外のキーを JSON へ出せないため、失敗したら `YamlInput` で読み直す
    let json = match serde_norway::from_str::<Value>(text) {
        Ok(v) if contains_non_finite(&v) => {
            tracing::debug!(
                "JSON で表現できない数値 (.inf / .nan) を含むためスキップ (yaml_to_json)"
            );
            return Cow::Borrowed(text);
        }
        Ok(v) => serde_json::to_string_pretty(&v),
        Err(e) => {
            let Ok(YamlInput(v)) = serde_norway::from_str::<YamlInput>(text) else {
                tracing::debug!("YAML パースに失敗 (yaml_to_json): {}", e);
                return Cow::Borrowed(text);
            };
            serde_json::to_string_pretty(&v)
        }
    };

    match json {
        Ok(json_text) => Cow::Owned(json_text),
        Err(e) => {
            tracing::debug!("JSON 変換に失敗 (yaml_to_json): {}", e);
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
/// * `Cow<'_, str>` - 変換後のJSON文字列。パース・変換に失敗した場合 (`.inf` / `.nan` を含む場合など) や
///   入力がパーサの上限を超える場合は元の文字列を返す
pub fn yaml_to_json_preserve_order(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("YAML 入力が上限を超えているためスキップ (yaml_to_json_preserve_order)");
        return Cow::Borrowed(text);
    }

    let v = match serde_norway::from_str::<YamlInput>(text) {
        Ok(YamlInput(v)) => v,
        Err(e) => {
            tracing::debug!("YAML パースに失敗 (yaml_to_json_preserve_order): {}", e);
            return Cow::Borrowed(text);
        }
    };

    match serde_json::to_string_pretty(&v) {
        Ok(json) => Cow::Owned(json),
        Err(e) => {
            tracing::debug!("JSON 変換に失敗 (yaml_to_json_preserve_order): {}", e);
            Cow::Borrowed(text)
        }
    }
}

// ======================================================================
// YAML の読み取り
// ======================================================================
/// YAML をキー順序を保って `OrderedValue` へ読み取るためのラッパー
///
/// `OrderedValue` の派生実装 (`untagged`) は値をいったん serde 内部の型へ溜めるため、
/// `u64` / `i64` を超える整数を扱えず、数値や真偽値のキー (`1: x`) も読めない
/// 整数は `u128` / `i128` の範囲まで桁を保ち、キーは文字列として読む
struct YamlInput(OrderedValue);

impl<'de> Deserialize<'de> for YamlInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(YamlInputVisitor)
            .map(YamlInput)
    }
}

/// `YamlInput` の読み取り処理
struct YamlInputVisitor;

impl<'de> Visitor<'de> for YamlInputVisitor {
    type Value = OrderedValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("JSON で表現できる YAML の値")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(OrderedValue::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(OrderedValue::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(OrderedValue::Number(v.into()))
    }

    fn visit_i128<E: serde::de::Error>(self, v: i128) -> Result<Self::Value, E> {
        integer_to_number(&v.to_string())
    }

    fn visit_u128<E: serde::de::Error>(self, v: u128) -> Result<Self::Value, E> {
        integer_to_number(&v.to_string())
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
        // `.inf` / `.nan` は JSON で表現できず、null にすると値が変わるため変換しない
        Number::from_f64(v)
            .map(OrderedValue::Number)
            .ok_or_else(|| E::custom("JSON で表現できない数値 (.inf / .nan)"))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(OrderedValue::String(v.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
        Ok(OrderedValue::String(v))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(OrderedValue::Null)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(OrderedValue::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        YamlInput::deserialize(deserializer).map(|YamlInput(v)| v)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(YamlInput(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(OrderedValue::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut object = IndexMap::new();
        while let Some((key, YamlInput(value))) = map.next_entry::<String, YamlInput>()? {
            object.insert(key, value);
        }
        Ok(OrderedValue::Object(object))
    }
}

/// YAML の値が `.inf` / `.nan` などの有限でない数値を含むか判定する
///
/// JSON は有限でない数値を表現できず、そのまま直列化すると null になる
///
/// # Arguments
/// * `value` - 判定する YAML の値
///
/// # Returns
/// * `bool` - 有限でない数値を含む場合は `true`
fn contains_non_finite(value: &Value) -> bool {
    match value {
        Value::Number(n) => !n.is_finite(),
        Value::Sequence(items) => items.iter().any(contains_non_finite),
        Value::Mapping(map) => map
            .iter()
            .any(|(k, v)| contains_non_finite(k) || contains_non_finite(v)),
        Value::Tagged(tagged) => contains_non_finite(&tagged.value),
        _ => false,
    }
}

/// 10 進の整数表記を桁を保った JSON の数値にする
///
/// # Arguments
/// * `digits` - 整数の 10 進表記
///
/// # Returns
/// * `Result<OrderedValue, E>` - 数値。表記が JSON の数値として不正な場合はエラー
fn integer_to_number<E: serde::de::Error>(digits: &str) -> Result<OrderedValue, E> {
    digits
        .parse::<Number>()
        .map(OrderedValue::Number)
        .map_err(E::custom)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 有効な YAML を JSON に変換できるようにすること
    #[test]
    fn test_yaml_to_json_valid() {
        let input = "b: 1\na: 2\n";
        let output = yaml_to_json(input);
        assert!(output.contains("\"a\": 2"));
    }

    /// 不正な YAML は元の文字列を返すようにすること
    #[test]
    fn test_yaml_to_json_invalid() {
        let input = "a: 1\n  b: 2";
        assert_eq!(yaml_to_json(input), input);
    }

    /// 有効な YAML をキー順序を維持した JSON に変換できるようにすること
    #[test]
    fn test_yaml_to_json_preserve_order_valid() {
        let input = "b: 1\na: 2\n";
        let output = yaml_to_json_preserve_order(input);
        let b_pos = output.find("\"b\"").expect("b キーが含まれる");
        let a_pos = output.find("\"a\"").expect("a キーが含まれる");
        assert!(b_pos < a_pos, "YAML のキー順序が維持されていない");
    }

    /// 不正な YAML (キー順序保持版) は元の文字列を返すようにすること
    #[test]
    fn test_yaml_to_json_preserve_order_invalid() {
        let input = "a: 1\n  b: 2";
        assert_eq!(yaml_to_json_preserve_order(input), input);
    }

    /// `u64` / `i64` を超える整数を桁を保って JSON に変換すること
    #[test]
    fn test_yaml_to_json_keeps_large_integers() {
        let input = "- 18446744073709551616\n- -170141183460469231731687303715884105728\n";
        let expected = "[\n  18446744073709551616,\n  -170141183460469231731687303715884105728\n]";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
    }

    /// 数値や真偽値のキーを文字列のキーとして JSON に変換すること
    #[test]
    fn test_yaml_to_json_preserve_order_accepts_non_string_keys() {
        let output = yaml_to_json_preserve_order("2: b\ntrue: c\n1: a\n");
        assert_eq!(
            output,
            "{\n  \"2\": \"b\",\n  \"true\": \"c\",\n  \"1\": \"a\"\n}"
        );
    }

    /// タグ付きの値はキー順序不同の変換で従来どおり変換すること
    #[test]
    fn test_yaml_to_json_keeps_tagged_values() {
        let output = yaml_to_json("a: !Ref b\n");
        assert!(output.contains("\"!Ref\": \"b\""), "{output}");
    }

    /// `.inf` / `.nan` を含む YAML は null へ変えず元の文字列を返すようにすること
    #[test]
    fn test_yaml_to_json_rejects_non_finite_numbers() {
        for input in [
            "a: .inf
",
            "a: -.INF
",
            "a: .nan
",
            "- 1
- [.NaN]
",
            "a: !Tag .inf
",
            "a: {b: .Inf}
",
        ] {
            assert_eq!(yaml_to_json(input), input);
            assert_eq!(yaml_to_json_preserve_order(input), input);
        }
    }

    /// 有限の小数と、`.inf` に似た文字列は従来どおり変換すること
    #[test]
    fn test_yaml_to_json_keeps_finite_numbers_and_inf_like_strings() {
        let input = "a: 1.5
b: \".inf\"
c: .infinity
";
        let expected = "{
  \"a\": 1.5,
  \"b\": \".inf\",
  \"c\": \".infinity\"
}";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
    }

    /// パーサ上限超過の入力は JSON 変換をスキップすること
    #[test]
    fn test_yaml_to_json_skips_oversized_input() {
        use crate::consts::MAX_PARSER_INPUT_BYTES;

        let input = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
        let output = yaml_to_json(&input);
        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(output.len(), input.len());
    }

    /// パーサ上限超過の入力はキー順序保持でもスキップすること
    #[test]
    fn test_yaml_to_json_preserve_order_skips_oversized_input() {
        use crate::consts::MAX_PARSER_INPUT_BYTES;

        let input = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
        let output = yaml_to_json_preserve_order(&input);
        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(output.len(), input.len());
    }
}
