use std::borrow::Cow;
use std::fmt;

use indexmap::IndexMap;
use indexmap::map::Entry;
use serde::de::{
    Deserialize, DeserializeSeed, Deserializer, EnumAccess, Error as _, IgnoredAny, MapAccess,
    SeqAccess, VariantAccess, Visitor,
};
use serde_json::Number;

use super::exact_number::loses_digits_as_f64;
use super::utils::strip_bom;
use crate::refiner::OrderedValue;
use crate::security::is_within_parser_limit;

// ======================================================================
// 定数
// ======================================================================
/// `f64` に丸めると桁が変わりうる数値の最小の桁数
///
/// 15 桁以下の 10 進数は `f64` で往復できる
const MIN_LOSSY_DIGITS: usize = 16;

/// YAML 1.1 のマージキー
const MERGE_KEY: &str = "<<";

// ======================================================================
// YAML → JSON 変換
// ======================================================================
/// YAML文字列をJSON文字列へ変換する(キー順序不同)
///
/// 入力されたYAMLを解析し、対応するJSON形式の文字列に変換する
/// キーの順序は保証されない
/// `---` で区切った複数のドキュメントは配列にする。マージキー (`<<`) は展開し、タグは捨てて値だけを出力する
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のJSON文字列。パース・変換に失敗した場合 (`.inf` / `.nan` やキーの重複を含む場合など) や
///   入力がパーサの上限を超える場合は元の文字列を返す
pub fn yaml_to_json(text: &str) -> Cow<'_, str> {
    convert_to_json(text, "yaml_to_json")
}

/// YAML文字列をJSON文字列へ変換する(キー順序保持)
///
/// 入力されたYAMLを解析し、対応するJSON形式の文字列に変換する
/// 可能な限り、元のYAMLにおけるマップのキー順序を維持して変換を試みる
/// `---` で区切った複数のドキュメントは配列にする。マージキー (`<<`) は展開し、タグは捨てて値だけを出力する
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のJSON文字列。パース・変換に失敗した場合 (`.inf` / `.nan` やキーの重複を含む場合など) や
///   入力がパーサの上限を超える場合は元の文字列を返す
pub fn yaml_to_json_preserve_order(text: &str) -> Cow<'_, str> {
    convert_to_json(text, "yaml_to_json_preserve_order")
}

/// YAML を JSON へ変換する共通処理
///
/// # Arguments
/// * `text` - 変換対象のYAML文字列
/// * `name` - ログに出す変換名
///
/// # Returns
/// * `Cow<'a, str>` - 変換後のJSON文字列。失敗した場合や入力がパーサの上限を超える場合は元の文字列
fn convert_to_json<'a>(text: &'a str, name: &str) -> Cow<'a, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("YAML 入力が上限を超えているためスキップ ({name})");
        return Cow::Borrowed(text);
    }

    let value = match read_yaml(strip_bom(text)) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("YAML パースに失敗 ({name}): {e}");
            return Cow::Borrowed(text);
        }
    };

    match serde_json::to_string_pretty(&value) {
        Ok(json) => Cow::Owned(json),
        Err(e) => {
            tracing::debug!("JSON 変換に失敗 ({name}): {e}");
            Cow::Borrowed(text)
        }
    }
}

// ======================================================================
// YAML の読み取り
// ======================================================================
/// YAML を JSON で表現できる値へ読み取る
///
/// 複数のドキュメントは配列にまとめ、ドキュメントが無い場合は null とする
/// `f64` に丸めると桁が変わる数値は元の桁を保つ
///
/// # Arguments
/// * `text` - YAML 文字列
///
/// # Returns
/// * `Result<OrderedValue, serde_norway::Error>` - 読み取った値。YAML として不正な場合や、
///   JSON で表現できない値 (`.inf` / `.nan`)・重複キーを含む場合はエラー
fn read_yaml(text: &str) -> Result<OrderedValue, serde_norway::Error> {
    let mut documents = Vec::new();
    for document in serde_norway::Deserializer::from_str(text) {
        documents.push(YamlNode::deserialize(document)?);
    }
    if has_long_digit_run(text) {
        // 読み直せない形の場合は、`f64` に丸めた値のまま変換する
        let _ = keep_digits(text, &mut documents);
    }

    let mut values = documents
        .into_iter()
        .map(YamlNode::into_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match values.len() {
        0 => OrderedValue::Null,
        1 => values.remove(0),
        _ => OrderedValue::Array(values),
    })
}

/// YAML の 1 ドキュメントを、重複キーとマージキーを解決する前の形で読み取った値
///
/// `OrderedValue` の派生実装 (`untagged`) は値をいったん serde 内部の型へ溜めるため、
/// `u64` / `i64` を超える整数を扱えず、数値や真偽値のキー (`1: x`) も読めない
/// 整数は `u128` / `i128` の範囲まで桁を保ち、キーは文字列として読む
/// タグ (`!Ref` など) は捨てて、付けられた値だけを読む
enum YamlNode {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<YamlNode>),
    /// キーが重複しうるマッピング。出現順に持つ
    Map(Vec<(String, YamlNode)>),
}

impl<'de> Deserialize<'de> for YamlNode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(YamlNodeVisitor)
    }
}

/// `YamlNode` の読み取り処理
struct YamlNodeVisitor;

impl<'de> Visitor<'de> for YamlNodeVisitor {
    type Value = YamlNode;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("JSON で表現できる YAML の値")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(YamlNode::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(YamlNode::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(YamlNode::Number(v.into()))
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
            .map(YamlNode::Number)
            .ok_or_else(|| E::custom("JSON で表現できない数値 (.inf / .nan)"))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(YamlNode::String(v.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
        Ok(YamlNode::String(v))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(YamlNode::Null)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(YamlNode::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        YamlNode::deserialize(deserializer)
    }

    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Self::Value, A::Error> {
        // タグ付きの値 (`!Ref b`)。タグは読み捨てる
        let (IgnoredAny, variant) = data.variant()?;
        variant.newtype_variant()
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(YamlNode::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, YamlNode>()? {
            entries.push(entry);
        }
        Ok(YamlNode::Map(entries))
    }
}

impl YamlNode {
    /// 重複キーとマージキーを解決して `OrderedValue` にする
    ///
    /// マージキー (`<<`) の値がマッピング (またはマッピングだけの配列) の場合は、
    /// 明示したキーを優先して、足りないキーだけをオブジェクトの末尾に加える
    /// 配列では先頭に近いマッピングを優先する
    ///
    /// # Returns
    /// * `Result<OrderedValue, serde_norway::Error>` - 変換した値。同じオブジェクトにキーが重複している場合はエラー
    fn into_value(self) -> Result<OrderedValue, serde_norway::Error> {
        match self {
            Self::Null => Ok(OrderedValue::Null),
            Self::Bool(b) => Ok(OrderedValue::Bool(b)),
            Self::Number(n) => Ok(OrderedValue::Number(n)),
            Self::String(s) => Ok(OrderedValue::String(s)),
            Self::Array(items) => items
                .into_iter()
                .map(Self::into_value)
                .collect::<Result<_, _>>()
                .map(OrderedValue::Array),
            Self::Map(entries) => {
                let mut object = IndexMap::new();
                let mut merge = None;
                for (key, node) in entries {
                    let value = node.into_value()?;
                    if key == MERGE_KEY && is_merge_source(&value) {
                        if merge.replace(value).is_some() {
                            return Err(duplicate_key_error(&key));
                        }
                        continue;
                    }
                    match object.entry(key) {
                        Entry::Occupied(entry) => return Err(duplicate_key_error(entry.key())),
                        Entry::Vacant(entry) => {
                            entry.insert(value);
                        }
                    }
                }
                match merge {
                    Some(OrderedValue::Object(source)) => merge_missing(&mut object, source),
                    Some(OrderedValue::Array(sources)) => {
                        for source in sources {
                            if let OrderedValue::Object(source) = source {
                                merge_missing(&mut object, source);
                            }
                        }
                    }
                    _ => {}
                }
                Ok(OrderedValue::Object(object))
            }
        }
    }
}

/// マージキー (`<<`) の値としてマッピングを取り込める形か判定する
///
/// マッピング、またはマッピングだけを持つ空でない配列が対象。それ以外は通常のキーとして扱う
fn is_merge_source(value: &OrderedValue) -> bool {
    match value {
        OrderedValue::Object(_) => true,
        OrderedValue::Array(items) => {
            !items.is_empty()
                && items
                    .iter()
                    .all(|item| matches!(item, OrderedValue::Object(_)))
        }
        _ => false,
    }
}

/// `source` のうち `object` に無いキーだけを `object` の末尾に加える
fn merge_missing(
    object: &mut IndexMap<String, OrderedValue>,
    source: IndexMap<String, OrderedValue>,
) {
    for (key, value) in source {
        object.entry(key).or_insert(value);
    }
}

/// 重複キーのエラーを作る
fn duplicate_key_error(key: &str) -> serde_norway::Error {
    serde_norway::Error::custom(format!("キーが重複している: {key}"))
}

/// 10 進の整数表記を桁を保った JSON の数値にする
///
/// # Arguments
/// * `digits` - 整数の 10 進表記
///
/// # Returns
/// * `Result<YamlNode, E>` - 数値。表記が JSON の数値として不正な場合はエラー
fn integer_to_number<E: serde::de::Error>(digits: &str) -> Result<YamlNode, E> {
    digits
        .parse::<Number>()
        .map(YamlNode::Number)
        .map_err(E::custom)
}

// ======================================================================
// 数値の桁の保持
// ======================================================================
/// `f64` に丸めると桁が変わる数値を、元の表記のまま読み取り直す
///
/// `serde_norway` は数値を `u128` / `i128` / `f64` でしか渡さないため、
/// いったん `YamlNode` で読んだあと、数値だけ文字列として読み直して元の表記を取り出す
///
/// # Arguments
/// * `text` - YAML 文字列
/// * `documents` - 読み取り済みの各ドキュメント。数値を元の表記で置き換える
///
/// # Returns
/// * `Result<(), serde_norway::Error>` - 読み直せない形の場合はエラー
fn keep_digits(text: &str, documents: &mut [YamlNode]) -> Result<(), serde_norway::Error> {
    for (node, document) in documents
        .iter_mut()
        .zip(serde_norway::Deserializer::from_str(text))
    {
        DigitsSeed(node).deserialize(document)?;
    }
    Ok(())
}

/// `MIN_LOSSY_DIGITS` 桁以上の数字の並び (小数点を挟んでよい) を含むか判定する
///
/// # Arguments
/// * `text` - 判定する文字列
///
/// # Returns
/// * `bool` - 含む場合は `true`
fn has_long_digit_run(text: &str) -> bool {
    let mut digits = 0;
    for b in text.bytes() {
        match b {
            b'0'..=b'9' => {
                digits += 1;
                if digits >= MIN_LOSSY_DIGITS {
                    return true;
                }
            }
            b'.' => {}
            _ => digits = 0,
        }
    }
    false
}

/// 読み取り済みの値の数値を、同じ YAML の元の表記へ置き換えるシード
///
/// 値の形 (配列・マッピング・キー) は読み取り済みの値と同じ順序で現れる前提とし、
/// 食い違う場合は読み取りを失敗させる
struct DigitsSeed<'a>(&'a mut YamlNode);

impl<'de> DeserializeSeed<'de> for DigitsSeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        match self.0 {
            YamlNode::Number(n) => deserializer.deserialize_str(DigitsNumberVisitor(n)),
            YamlNode::Array(items) => deserializer.deserialize_seq(DigitsSeqVisitor(items)),
            YamlNode::Map(entries) => deserializer.deserialize_map(DigitsMapVisitor(entries)),
            _ => deserializer.deserialize_ignored_any(IgnoredAny).map(|_| ()),
        }
    }
}

/// 数値のスカラーを元の表記で置き換える
struct DigitsNumberVisitor<'a>(&'a mut Number);

impl Visitor<'_> for DigitsNumberVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("数値のスカラー")
    }

    fn visit_str<E>(self, v: &str) -> Result<(), E> {
        // JSON の数値として書けない表記 (`0x1F`・`+1`・`.5` など) は丸めた値のまま残す
        if let (Some(rounded), Ok(exact)) = (self.0.as_f64(), v.parse::<Number>())
            && loses_digits_as_f64(v, rounded)
        {
            *self.0 = exact;
        }
        Ok(())
    }
}

/// 配列の要素を順に置き換える
struct DigitsSeqVisitor<'a>(&'a mut [YamlNode]);

impl<'de> Visitor<'de> for DigitsSeqVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("配列")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        for item in self.0.iter_mut() {
            seq.next_element_seed(DigitsSeed(item))?
                .ok_or_else(|| A::Error::custom("配列の要素数が一致しない"))?;
        }
        match seq.next_element::<IgnoredAny>()? {
            None => Ok(()),
            Some(_) => Err(A::Error::custom("配列の要素数が一致しない")),
        }
    }
}

/// マッピングの値を順に置き換える
struct DigitsMapVisitor<'a>(&'a mut [(String, YamlNode)]);

impl<'de> Visitor<'de> for DigitsMapVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("マッピング")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        for (expected, value) in self.0.iter_mut() {
            match map.next_key::<String>()? {
                Some(key) if key == *expected => {}
                _ => return Err(A::Error::custom("マッピングのキーが一致しない")),
            }
            map.next_value_seed(DigitsSeed(value))?;
        }
        match map.next_key::<IgnoredAny>()? {
            None => Ok(()),
            Some(_) => Err(A::Error::custom("マッピングのキー数が一致しない")),
        }
    }
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

    /// 倍精度を超える整数・小数を元の桁のまま JSON に変換すること
    #[test]
    fn test_yaml_to_json_keeps_digits_beyond_f64() {
        let input = "a: 123456789012345678901234567890123456789012\n\
                     b: 3.141592653589793238462643\n\
                     c: [1.5, -0.1234567890123456789, 9007199254740993]\n\
                     d: {e: 1.0000000000000000001e5, f: '3.141592653589793238462643'}\n";
        let expected = "{\n  \"a\": 123456789012345678901234567890123456789012,\n  \
                        \"b\": 3.141592653589793238462643,\n  \
                        \"c\": [\n    1.5,\n    -0.1234567890123456789,\n    9007199254740993\n  ],\n  \
                        \"d\": {\n    \"e\": 1.0000000000000000001e+5,\n    \
                        \"f\": \"3.141592653589793238462643\"\n  }\n}";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
    }

    /// 桁が変わらない長い数値は従来どおりの表記で変換すること
    #[test]
    fn test_yaml_to_json_keeps_long_exact_numbers_as_before() {
        let input = "a: 1.50000000000000000000\nb: 0x1F\nc: 1234567890123456\n";
        let expected = "{\n  \"a\": 1.5,\n  \"b\": 31,\n  \"c\": 1234567890123456\n}";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
    }

    /// タグ付きの値でも桁を保って変換すること
    #[test]
    fn test_yaml_to_json_keeps_digits_in_tagged_values() {
        let input = "a: !Ref 3.141592653589793238462643\nb: !Tag [18446744073709551616]\n";
        let expected =
            "{\n  \"a\": 3.141592653589793238462643,\n  \"b\": [\n    18446744073709551616\n  ]\n}";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
    }

    /// 重複キーを含む YAML は、桁の長い数値があっても元の文字列を返すこと
    #[test]
    fn test_yaml_to_json_rejects_duplicate_keys_with_long_numbers() {
        let input = "a: 1\na: 3.141592653589793238462643\n";
        assert_eq!(yaml_to_json(input), input);
        assert_eq!(yaml_to_json_preserve_order(input), input);
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

    /// タグを捨てて、付けられた値だけを JSON に変換すること
    #[test]
    fn test_yaml_to_json_drops_tags() {
        for input in ["a: !Ref b\n", "a: !custom b\n"] {
            let expected = "{\n  \"a\": \"b\"\n}";
            assert_eq!(yaml_to_json(input), expected);
            assert_eq!(yaml_to_json_preserve_order(input), expected);
        }
        assert_eq!(yaml_to_json("!custom 1\n"), "1");
        assert_eq!(
            yaml_to_json("a: !Seq [1, !Inner 2]\nb: !Map {c: !Str d}\n"),
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": \"d\"\n  }\n}"
        );
    }

    /// `---` で区切った複数のドキュメントを配列にして変換すること
    #[test]
    fn test_yaml_to_json_collects_multiple_documents() {
        let input = "a: 1\n---\n- x\n---\n3\n";
        let expected = "[\n  {\n    \"a\": 1\n  },\n  [\n    \"x\"\n  ],\n  3\n]";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
        // 桁を保つ数値も、ドキュメントをまたいで元の表記になる
        assert_eq!(
            yaml_to_json("1.5\n---\n3.141592653589793238462643\n"),
            "[\n  1.5,\n  3.141592653589793238462643\n]"
        );
    }

    /// ドキュメントが 1 つだけなら、先頭の `---` や末尾の `...` があっても配列にしないこと
    #[test]
    fn test_yaml_to_json_single_document_is_not_wrapped() {
        assert_eq!(yaml_to_json("---\na: 1\n"), "{\n  \"a\": 1\n}");
        assert_eq!(yaml_to_json("---\na: 1\n...\n"), "{\n  \"a\": 1\n}");
    }

    /// 空の入力やコメントだけの入力は null として変換すること
    #[test]
    fn test_yaml_to_json_empty_input_is_null() {
        assert_eq!(yaml_to_json(""), "null");
        assert_eq!(yaml_to_json("# comment\n"), "null");
    }

    /// 同じマッピング内の重複キーは変換せず元の文字列を返すこと
    #[test]
    fn test_yaml_to_json_rejects_duplicate_keys() {
        for input in [
            "a: 1\na: 2\n",
            "x:\n  b: 1\n  b: 2\n",
            "- {a: 1, a: 2}\n",
            "1: x\n'1': y\n",
            "<<: {a: 1}\n<<: {b: 2}\n",
        ] {
            assert_eq!(yaml_to_json(input), input);
            assert_eq!(yaml_to_json_preserve_order(input), input);
        }
        // 別々のマッピングに同じキーがあっても重複ではない
        assert!(matches!(yaml_to_json("- a: 1\n- a: 2\n"), Cow::Owned(_)));
    }

    /// マージキーを展開し、明示したキーを優先して足りないキーを末尾に加えること
    #[test]
    fn test_yaml_to_json_expands_merge_keys() {
        let input = "base: &base\n  a: 1\n  b: 2\nchild:\n  <<: *base\n  b: 3\n  c: 4\n";
        let expected = "{\n  \"base\": {\n    \"a\": 1,\n    \"b\": 2\n  },\n  \
                        \"child\": {\n    \"b\": 3,\n    \"c\": 4,\n    \"a\": 1\n  }\n}";
        assert_eq!(yaml_to_json(input), expected);
        assert_eq!(yaml_to_json_preserve_order(input), expected);
    }

    /// マージキーに配列を渡すと、先頭に近いマッピングを優先して取り込むこと
    #[test]
    fn test_yaml_to_json_expands_merge_key_sequence() {
        let input = "x: &x {a: 1, b: 1}\ny: &y {b: 2, c: 2}\nz:\n  <<: [*x, *y]\n  d: 4\n";
        let output = yaml_to_json(input);
        let value: serde_json::Value = serde_json::from_str(&output).expect("JSON として読める");
        assert_eq!(
            value["z"],
            serde_json::json!({"a": 1, "b": 1, "c": 2, "d": 4})
        );
        assert!(!output.contains("<<"), "{output}");
    }

    /// マッピングを取り込めない `<<` は通常のキーとして残すこと
    #[test]
    fn test_yaml_to_json_keeps_non_mapping_merge_key() {
        assert_eq!(yaml_to_json("<<: 1\n"), "{\n  \"<<\": 1\n}");
        assert_eq!(
            yaml_to_json("<<: [1, 2]\n"),
            "{\n  \"<<\": [\n    1,\n    2\n  ]\n}"
        );
    }

    /// 先頭の BOM を無視して変換すること
    #[test]
    fn test_yaml_to_json_ignores_leading_bom() {
        assert_eq!(yaml_to_json("\u{FEFF}a: 1\n"), "{\n  \"a\": 1\n}");
        assert_eq!(
            yaml_to_json_preserve_order("\u{FEFF}a: 1\n"),
            "{\n  \"a\": 1\n}"
        );
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
