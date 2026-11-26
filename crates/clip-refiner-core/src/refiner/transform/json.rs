use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;

use serde::de::{DeserializeOwned, Deserializer, Error as _, MapAccess, SeqAccess, Visitor};
use serde::ser::{Error as _, Serialize, Serializer};
use serde_json::error::Category;
use serde_json::{Number, Value};

use super::exact_number::loses_digits_as_f64;
use super::utils::strip_bom;
use crate::refiner::{OrderedValue, ParseFailure};
use crate::security::is_within_parser_limit;

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
/// * `Result<Cow<'_, str>, ParseFailure>` - 整形済みのJSON文字列。すでに整形済みの場合や入力がパーサの上限を超える場合は元の文字列を返す
///   パースに失敗した場合 (キーが重複している場合を含む) はその理由を返す
pub fn format_json(text: &str) -> Result<Cow<'_, str>, ParseFailure> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (format_json)");
        return Ok(Cow::Borrowed(text));
    }

    let v: Value = parse_json(text).inspect_err(|f| {
        tracing::debug!("JSON パースに失敗 (format_json): {f:?}");
    })?;

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => {
            if pretty == text {
                Ok(Cow::Borrowed(text))
            } else {
                Ok(Cow::Owned(pretty))
            }
        }
        Err(e) => {
            tracing::debug!("JSON シリアライズに失敗 (format_json): {}", e);
            Err(ParseFailure::UnrepresentableValue)
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
/// * `Result<Cow<'_, str>, ParseFailure>` - 整形済みのJSON文字列。すでに整形済みの場合や入力がパーサの上限を超える場合は元の文字列を返す
///   パースに失敗した場合 (キーが重複している場合を含む) はその理由を返す
pub fn format_json_preserve_order(text: &str) -> Result<Cow<'_, str>, ParseFailure> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (format_json_preserve_order)");
        return Ok(Cow::Borrowed(text));
    }

    let v: OrderedValue = parse_json(text).inspect_err(|f| {
        tracing::debug!("JSON パースに失敗 (format_json_preserve_order): {f:?}");
    })?;

    match serde_json::to_string_pretty(&v) {
        Ok(pretty) => {
            if pretty == text {
                Ok(Cow::Borrowed(text))
            } else {
                Ok(Cow::Owned(pretty))
            }
        }
        Err(e) => {
            tracing::debug!(
                "JSON シリアライズに失敗 (format_json_preserve_order): {}",
                e
            );
            Err(ParseFailure::UnrepresentableValue)
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
/// オブジェクトでも配列でもない入力 (文字列や数値だけの JSON) は、引用符を外したり改行を足したりするだけになるため変換しない
///
/// # Arguments
/// * `text` - 変換対象のJSON文字列
///
/// # Returns
/// * `Result<Cow<'_, str>, ParseFailure>` - 変換後のYAML文字列。オブジェクトでも配列でもない場合や入力がパーサの上限を超える場合は元の文字列を返す
///   パース・変換に失敗した場合 (キーが重複している場合や YAML で表現できない数値を含む場合など) はその理由を返す
pub fn json_to_yaml(text: &str) -> Result<Cow<'_, str>, ParseFailure> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (json_to_yaml)");
        return Ok(Cow::Borrowed(text));
    }

    let v: Value = parse_json(text).inspect_err(|f| {
        tracing::debug!("JSON パースに失敗 (json_to_yaml): {f:?}");
    })?;
    if !(v.is_object() || v.is_array()) {
        tracing::debug!("JSON がオブジェクトでも配列でもないためスキップ (json_to_yaml)");
        return Ok(Cow::Borrowed(text));
    }

    match to_yaml_string(|guard| serde_norway::to_string(&YamlValue { value: &v, guard })) {
        Ok(yaml_text) => Ok(Cow::Owned(yaml_text)),
        Err(e) => {
            tracing::debug!("YAML 変換に失敗 (json_to_yaml): {}", e);
            Err(ParseFailure::UnrepresentableValue)
        }
    }
}

/// JSON文字列をYAML文字列へ変換する(キー順序保持)
///
/// 入力されたJSONを解析し、対応するYAML形式の文字列に変換する
/// 元のJSONに含まれるオブジェクトのキーの順序を可能な限り維持する
/// オブジェクトでも配列でもない入力 (文字列や数値だけの JSON) は、引用符を外したり改行を足したりするだけになるため変換しない
///
/// # Arguments
/// * `text` - 変換対象のJSON文字列
///
/// # Returns
/// * `Result<Cow<'_, str>, ParseFailure>` - 変換後のYAML文字列。オブジェクトでも配列でもない場合や入力がパーサの上限を超える場合は元の文字列を返す
///   パース・変換に失敗した場合 (キーが重複している場合や YAML で表現できない数値を含む場合など) はその理由を返す
pub fn json_to_yaml_preserve_order(text: &str) -> Result<Cow<'_, str>, ParseFailure> {
    if !is_within_parser_limit(text) {
        tracing::debug!("JSON 入力が上限を超えているためスキップ (json_to_yaml_preserve_order)");
        return Ok(Cow::Borrowed(text));
    }

    let v: OrderedValue = parse_json(text).inspect_err(|f| {
        tracing::debug!("JSON パースに失敗 (json_to_yaml_preserve_order): {f:?}");
    })?;
    if !matches!(v, OrderedValue::Object(_) | OrderedValue::Array(_)) {
        tracing::debug!(
            "JSON がオブジェクトでも配列でもないためスキップ (json_to_yaml_preserve_order)"
        );
        return Ok(Cow::Borrowed(text));
    }

    match to_yaml_string(|guard| serde_norway::to_string(&YamlOrderedValue { value: &v, guard })) {
        Ok(yaml_text) => Ok(Cow::Owned(yaml_text)),
        Err(e) => {
            tracing::debug!("YAML 変換に失敗 (json_to_yaml_preserve_order): {}", e);
            Err(ParseFailure::UnrepresentableValue)
        }
    }
}

// ======================================================================
// JSON の読み取り
// ======================================================================
/// JSON 文字列を値へ読み取る
///
/// 先頭の BOM は無視する。重複したキーを持つオブジェクトは、後の値で黙って上書きされないよう失敗させる
///
/// # Arguments
/// * `text` - 読み取る JSON 文字列
///
/// # Returns
/// * `Result<T, ParseFailure>` - 読み取った値。不正な JSON なら `InvalidJson`、重複キーを含むなら `DuplicateKey`
fn parse_json<T: DeserializeOwned>(text: &str) -> Result<T, ParseFailure> {
    let text = strip_bom(text);
    // 検査用の読み取りで構文以外のエラーになるのは、重複キーを検出した場合だけ
    serde_json::from_str::<NoDuplicateKeys>(text).map_err(|e| match e.classify() {
        Category::Data => ParseFailure::DuplicateKey,
        _ => ParseFailure::InvalidJson,
    })?;
    serde_json::from_str(text).map_err(|_| ParseFailure::InvalidJson)
}

/// 値を読み捨てながら、同じオブジェクト内の重複キーを検出する検査用の型
struct NoDuplicateKeys;

impl<'de> serde::Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NoDuplicateKeysVisitor)
    }
}

/// `NoDuplicateKeys` の読み取り処理
///
/// `arbitrary_precision` 有効時の数値は 1 キーだけのオブジェクトとして渡されるが、重複は起きない
struct NoDuplicateKeysVisitor;

impl<'de> Visitor<'de> for NoDuplicateKeysVisitor {
    type Value = NoDuplicateKeys;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("JSON の値")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<NoDuplicateKeys>()?.is_some() {}
        Ok(NoDuplicateKeys)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut seen = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if seen.contains(&key) {
                return Err(A::Error::custom(format!("キーが重複している: {key}")));
            }
            seen.insert(key);
            map.next_value::<NoDuplicateKeys>()?;
        }
        Ok(NoDuplicateKeys)
    }
}

// ======================================================================
// YAML 出力用の値変換
// ======================================================================
/// YAML 1.1 で真偽値として解釈される文字列
///
/// `serde_norway` は YAML 1.2 の `true` / `false` しか引用符で囲わないため、
/// これらは出力時に自前で単一引用符を付ける
const YAML11_BOOL_WORDS: [&str; 16] = [
    "y", "Y", "n", "N", "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off",
    "OFF",
];

/// YAML 1.1 で特別な意味を持つ文字列
///
/// `<<` はマージキー、`=` は value タグとして解釈される
const YAML11_SPECIAL_WORDS: [&str; 2] = ["<<", "="];

/// 先頭から `min` 〜 `max` 桁の ASCII 数字を読み、残りを返す
fn skip_digits(s: &str, min: usize, max: usize) -> Option<&str> {
    let len = s.bytes().take_while(u8::is_ascii_digit).count().min(max);
    (len >= min).then(|| &s[len..])
}

/// YAML 1.1 の日付 (`2024-01-01`) として解釈される文字列か
fn is_yaml11_date(s: &str) -> bool {
    skip_digits(s, 4, 4)
        .and_then(|r| r.strip_prefix('-'))
        .and_then(|r| skip_digits(r, 1, 2))
        .and_then(|r| r.strip_prefix('-'))
        .and_then(|r| skip_digits(r, 1, 2))
        .is_some_and(str::is_empty)
}

/// YAML 1.1 の timestamp (`2024-01-01T00:00:00Z` など) として解釈される文字列か
///
/// 日付のみの文字列は含まない。区切りは `T` / `t` / 空白 1 文字、時刻の後は
/// `Z` またはオフセット (`+09:00` / `-5`) を任意で許す
fn is_yaml11_timestamp(s: &str) -> bool {
    let Some(rest) = skip_digits(s, 4, 4)
        .and_then(|r| r.strip_prefix('-'))
        .and_then(|r| skip_digits(r, 1, 2))
        .and_then(|r| r.strip_prefix('-'))
        .and_then(|r| skip_digits(r, 1, 2))
    else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(['T', 't', ' ']) else {
        return false;
    };
    let Some(mut rest) = skip_digits(rest, 1, 2)
        .and_then(|r| r.strip_prefix(':'))
        .and_then(|r| skip_digits(r, 2, 2))
        .and_then(|r| r.strip_prefix(':'))
        .and_then(|r| skip_digits(r, 2, 2))
    else {
        return false;
    };
    if let Some(frac) = rest.strip_prefix('.') {
        rest = frac.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    if rest.is_empty() || rest == "Z" {
        return true;
    }
    rest.strip_prefix(['+', '-'])
        .and_then(|r| skip_digits(r, 1, 2))
        .is_some_and(|r| {
            r.is_empty()
                || r.strip_prefix(':')
                    .and_then(|r| skip_digits(r, 2, 2))
                    .is_some_and(str::is_empty)
        })
}

/// YAML 1.1 の `_` 区切り数値 (`1_000` / `0x_FF` / `0b1_0` / `1_0.5` / `1_0.5e+3`) として解釈される文字列か
///
/// 浮動小数点の指数部は符号が必須 (`e+3` / `E-3`)
fn is_yaml11_underscore_number(s: &str) -> bool {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if !body.contains('_') {
        return false;
    }
    if let Some(hex) = body.strip_prefix("0x") {
        return hex.bytes().all(|b| b.is_ascii_hexdigit() || b == b'_');
    }
    if let Some(bin) = body.strip_prefix("0b") {
        return bin.bytes().all(|b| matches!(b, b'0' | b'1' | b'_'));
    }
    // 指数部は小数点を含む浮動小数点にだけ付く
    let body = match body.split_once(['e', 'E']) {
        Some((mantissa, exp)) => {
            let digits = exp.strip_prefix(['+', '-']);
            if !mantissa.contains('.') || !digits.is_some_and(is_ascii_digits) {
                return false;
            }
            mantissa
        }
        None => body,
    };
    body.starts_with(|c: char| c.is_ascii_digit())
        && body
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'_' | b'.'))
        && body.bytes().filter(|&b| b == b'.').count() <= 1
}

/// 1 文字以上の ASCII 数字だけで構成されているか
fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// YAML 1.1 の 60 進数 (`1:30` / `190:20:30.15`) として解釈される文字列か
fn is_yaml11_sexagesimal(s: &str) -> bool {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    let mut parts = body.split(':');
    let Some(head) = parts.next() else {
        return false;
    };
    let head_ok = head.starts_with(|c: char| matches!(c, '1'..='9'))
        && head.bytes().all(|b| b.is_ascii_digit() || b == b'_');
    if !head_ok {
        return false;
    }
    let tail: Vec<&str> = parts.collect();
    if tail.is_empty() {
        return false;
    }
    tail.iter().enumerate().all(|(i, part)| {
        // 小数部は最後の要素にだけ付けられる
        let digits = match part.split_once('.') {
            Some((int, frac)) if i + 1 == tail.len() => {
                if !frac.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
                    return false;
                }
                int
            }
            Some(_) => return false,
            None => part,
        };
        let b = digits.as_bytes();
        match b.len() {
            1 => b[0].is_ascii_digit(),
            2 => (b'0'..=b'5').contains(&b[0]) && b[1].is_ascii_digit(),
            _ => false,
        }
    })
}

/// YAML 1.1 のパーサーが文字列以外の型として解釈する文字列か
///
/// 真偽値語・日付・timestamp・`_` 区切り数値・60 進数が対象
fn is_yaml11_non_string(s: &str) -> bool {
    YAML11_BOOL_WORDS.contains(&s)
        || YAML11_SPECIAL_WORDS.contains(&s)
        || is_yaml11_date(s)
        || is_yaml11_timestamp(s)
        || is_yaml11_underscore_number(s)
        || is_yaml11_sexagesimal(s)
}

/// 引用符を付ける文字列を、直列化の間だけ別の語へ置き換えるための状態
///
/// `serde_norway` は文字列の引用スタイルを指定できない。そのため YAML 1.1 で文字列以外の型として
/// 読まれる語 (真偽値・日付など) は入力に現れない接頭辞付きの語へ置き換えて直列化し、
/// 出力後に `'yes'` の形へ戻す
///
/// `f64` に丸めると桁が変わる数値も同様に置き換え、出力後に元の表記へ戻す
struct QuoteGuard {
    prefix: String,
    /// 置き換えた元の文字列 (重複なし)
    masked: RefCell<HashSet<String>>,
    /// 置き換えた数値の YAML 表記 (重複なし)
    masked_numbers: RefCell<HashSet<String>>,
    /// 入力の文字列が接頭辞を含んでいたか
    collided: Cell<bool>,
}

impl QuoteGuard {
    fn new(prefix: String) -> Self {
        Self {
            prefix,
            masked: RefCell::new(HashSet::new()),
            masked_numbers: RefCell::new(HashSet::new()),
            collided: Cell::new(false),
        }
    }

    /// 桁を保つ数値を、直列化する文字列へ置き換える
    ///
    /// 接頭辞のあとに `num_` を付けて、文字列の置き換えと区別する
    ///
    /// # Arguments
    /// * `yaml_number` - 出力に使う数値の YAML 表記
    ///
    /// # Returns
    /// * `String` - 直列化する文字列
    fn mask_number(&self, yaml_number: &str) -> String {
        let mut masked = self.masked_numbers.borrow_mut();
        if !masked.contains(yaml_number) {
            masked.insert(yaml_number.to_owned());
        }
        format!("{}num_{yaml_number}__", self.prefix)
    }

    /// 直列化する文字列を返す (YAML 1.1 で文字列以外に読まれる語は置き換える)
    ///
    /// # Arguments
    /// * `s` - 元の文字列
    ///
    /// # Returns
    /// * `Cow<'a, str>` - 直列化する文字列
    fn mask<'a>(&self, s: &'a str) -> Cow<'a, str> {
        if s.contains(&self.prefix) {
            self.collided.set(true);
        }
        if is_yaml11_non_string(s) {
            let mut masked = self.masked.borrow_mut();
            if !masked.contains(s) {
                masked.insert(s.to_owned());
            }
            Cow::Owned(format!("{}{s}__", self.prefix))
        } else {
            Cow::Borrowed(s)
        }
    }

    /// 置き換えた語を単一引用符付きの文字列へ、置き換えた数値を元の表記へ戻す
    ///
    /// # Arguments
    /// * `yaml` - 直列化済みの YAML
    ///
    /// # Returns
    /// * `String` - 引用符を付けた YAML
    fn restore(&self, yaml: String) -> String {
        let masked = self.masked.borrow();
        let masked_numbers = self.masked_numbers.borrow();
        if masked.is_empty() && masked_numbers.is_empty() {
            return yaml;
        }
        // 接頭辞の出現位置ごとに、末尾の `__` までの語が置き換え済みの語かを引く (1 回の走査)
        let mut out = String::with_capacity(yaml.len());
        let mut rest = yaml.as_str();
        while let Some(pos) = rest.find(&self.prefix) {
            out.push_str(&rest[..pos]);
            let after = &rest[pos + self.prefix.len()..];
            if let Some((replacement, consumed)) = match_token(after, &masked, &masked_numbers) {
                out.push_str(&replacement);
                rest = &after[consumed..];
            } else {
                out.push_str(&self.prefix);
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }
}

/// 接頭辞の直後から、置き換えた語・数値に一致する先頭のトークンを探す
///
/// # Arguments
/// * `after` - 接頭辞の直後の文字列
/// * `masked` - 置き換えた語の集合
/// * `masked_numbers` - 置き換えた数値表記の集合
///
/// # Returns
/// * `Option<(String, usize)>` - 戻し後の文字列と、`after` から消費するバイト数
fn match_token(
    after: &str,
    masked: &HashSet<String>,
    masked_numbers: &HashSet<String>,
) -> Option<(String, usize)> {
    let num_part = after.strip_prefix("num_");
    let bytes = after.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'_' {
            i += 1;
            continue;
        }
        // 終端の `__` は連続する `_` の末尾 2 文字 (語自体が `_` で終わる場合を区別する)
        let start = i;
        while i < bytes.len() && bytes[i] == b'_' {
            i += 1;
        }
        if i - start < 2 {
            continue;
        }
        let idx = i - 2;
        if let Some(body) = num_part
            && idx >= 4
            && let Some(number) = body.get(..idx - 4)
            && masked_numbers.contains(number)
        {
            return Some((number.to_owned(), idx + 2));
        }
        if masked.contains(&after[..idx]) {
            return Some((format!("'{}'", &after[..idx]), idx + 2));
        }
    }
    None
}

/// YAML 文字列への直列化を、YAML 1.1 で文字列以外に読まれる語を引用符で囲って行う
///
/// 入力が置き換え用の接頭辞を含む場合は、接頭辞を伸ばして直列化をやり直す
///
/// # Arguments
/// * `serialize` - `QuoteGuard` を使って値を直列化する関数
///
/// # Returns
/// * `Result<String, serde_norway::Error>` - 直列化した YAML
fn to_yaml_string(
    serialize: impl Fn(&QuoteGuard) -> Result<String, serde_norway::Error>,
) -> Result<String, serde_norway::Error> {
    let mut prefix = String::from("__clip_refiner_yaml11_");
    loop {
        let guard = QuoteGuard::new(prefix.clone());
        let yaml = serialize(&guard)?;
        if !guard.collided.get() {
            return Ok(guard.restore(yaml));
        }
        prefix.push('_');
    }
}

/// `serde_json::Value` を YAML の数値・文字列表現で直列化するラッパー
///
/// `arbitrary_precision` 有効時の `Number` は JSON 以外の直列化先で内部構造体として
/// 出力されるため、数値だけ `serialize_yaml_number` で整数・浮動小数点へ変換する
/// 文字列とキーは `QuoteGuard` で YAML 1.1 の真偽値語・日付などに引用符を付ける
struct YamlValue<'a> {
    value: &'a Value,
    guard: &'a QuoteGuard,
}

impl Serialize for YamlValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let guard = self.guard;
        match self.value {
            Value::Number(n) => serialize_yaml_number(n, guard, serializer),
            Value::String(s) => serializer.serialize_str(&guard.mask(s)),
            Value::Array(items) => {
                serializer.collect_seq(items.iter().map(|value| YamlValue { value, guard }))
            }
            Value::Object(map) => serializer.collect_map(
                map.iter()
                    .map(|(k, value)| (YamlKey { key: k, guard }, YamlValue { value, guard })),
            ),
            other => other.serialize(serializer),
        }
    }
}

/// `OrderedValue` を YAML の数値・文字列表現で直列化するラッパー
///
/// 扱いは `YamlValue` と同じ
struct YamlOrderedValue<'a> {
    value: &'a OrderedValue,
    guard: &'a QuoteGuard,
}

impl Serialize for YamlOrderedValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let guard = self.guard;
        match self.value {
            OrderedValue::Number(n) => serialize_yaml_number(n, guard, serializer),
            OrderedValue::String(s) => serializer.serialize_str(&guard.mask(s)),
            OrderedValue::Array(items) => {
                serializer.collect_seq(items.iter().map(|value| YamlOrderedValue { value, guard }))
            }
            OrderedValue::Object(map) => {
                serializer.collect_map(map.iter().map(|(k, value)| {
                    (YamlKey { key: k, guard }, YamlOrderedValue { value, guard })
                }))
            }
            other => other.serialize(serializer),
        }
    }
}

/// オブジェクトのキーを `QuoteGuard` 経由で直列化するラッパー
struct YamlKey<'a> {
    key: &'a str,
    guard: &'a QuoteGuard,
}

impl Serialize for YamlKey<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.guard.mask(self.key))
    }
}

/// JSON の数値を YAML の整数・浮動小数点として直列化する
///
/// 整数は `u128` / `i128` の範囲まで桁を保つ。それ以外は `f64` で桁が変わらなければ `f64` として、
/// 変わる場合は `QuoteGuard` 経由で元の表記のまま出力する
/// `f64` で有限値にならない場合は変換失敗とする
///
/// # Arguments
/// * `n` - 直列化する数値
/// * `guard` - 桁を保つ数値の置き換え状態
/// * `serializer` - 直列化先
///
/// # Returns
/// * `Result<S::Ok, S::Error>` - 直列化結果
fn serialize_yaml_number<S: Serializer>(
    n: &Number,
    guard: &QuoteGuard,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let text = n.as_str();
    if let Ok(v) = text.parse::<u128>() {
        return serializer.serialize_u128(v);
    }
    if let Ok(v) = text.parse::<i128>() {
        return serializer.serialize_i128(v);
    }
    match n.as_f64() {
        Some(v) if v.is_finite() && loses_digits_as_f64(text, v) => {
            serializer.serialize_str(&guard.mask_number(&yaml_number_text(text)))
        }
        Some(v) if v.is_finite() => serializer.serialize_f64(v),
        _ => Err(S::Error::custom(format!("YAML で表現できない数値: {text}"))),
    }
}

/// JSON の数値表記を、YAML 1.1 のパーサーも浮動小数点として読める表記にする
///
/// 指数部を持つ場合は、仮数部に小数点を、指数部に符号を補う (`1e5` → `1.0e+5`)
///
/// # Arguments
/// * `text` - JSON の数値表記
///
/// # Returns
/// * `String` - YAML の数値表記
fn yaml_number_text(text: &str) -> String {
    let Some(i) = text.find(['e', 'E']) else {
        return text.to_owned();
    };
    let (mantissa, exponent) = (&text[..i], &text[i + 1..]);
    let dot = if mantissa.contains('.') { "" } else { ".0" };
    let sign = if exponent.starts_with(['+', '-']) {
        ""
    } else {
        "+"
    };
    format!("{mantissa}{dot}e{sign}{exponent}")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // 失敗時は元の文字列を返す形で検証できるよう、変換関数を包む
    // 失敗の理由は `rejects_with_failure_reason` で検証する
    fn format_json(text: &str) -> Cow<'_, str> {
        super::format_json(text).unwrap_or(Cow::Borrowed(text))
    }

    fn format_json_preserve_order(text: &str) -> Cow<'_, str> {
        super::format_json_preserve_order(text).unwrap_or(Cow::Borrowed(text))
    }

    fn json_to_yaml(text: &str) -> Cow<'_, str> {
        super::json_to_yaml(text).unwrap_or(Cow::Borrowed(text))
    }

    fn json_to_yaml_preserve_order(text: &str) -> Cow<'_, str> {
        super::json_to_yaml_preserve_order(text).unwrap_or(Cow::Borrowed(text))
    }

    /// 引用符を付ける語が大量にあっても YAML 化が線形時間で終わり、全てに引用符が付くこと
    #[test]
    fn json_to_yaml_quotes_many_dates_quickly() {
        let items: Vec<String> = (0..32_000)
            .map(|i| format!("\"2024-01-{:02}-{i}\"", i % 28 + 1))
            .chain((0..32_000).map(|i| format!("\"2024-{:02}-{:02}\"", i % 12 + 1, i % 28 + 1)))
            .collect();
        let input = format!("[{}]", items.join(","));
        let start = std::time::Instant::now();
        let output = json_to_yaml(&input);
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
        assert!(output.contains("- '2024-01-01'"));
        assert!(!output.contains("__clip_refiner_yaml11_"));
    }

    /// マージキー `<<` と value タグ `=` は、キーでも値でも引用符を付けること
    #[test]
    fn json_to_yaml_quotes_merge_key_and_value_tag() {
        let input = r#"{"<<":{"a":1},"b":"=","=":"<<"}"#;
        let expected = "'<<':\n  a: 1\n'=': '<<'\nb: '='\n";
        assert_eq!(json_to_yaml(input), expected);
        let expected = "'<<':\n  a: 1\nb: '='\n'=': '<<'\n";
        assert_eq!(json_to_yaml_preserve_order(input), expected);
    }

    /// `_` で終わる語が他の置き換え語と衝突しても、それぞれ元の語に戻ること
    #[test]
    fn json_to_yaml_restores_words_ending_with_underscore() {
        let input = r#"["1_","1__","1___","1_0"]"#;
        let expected = "- '1_'\n- '1__'\n- '1___'\n- '1_0'\n";
        assert_eq!(json_to_yaml(input), expected);
        assert_eq!(json_to_yaml_preserve_order(input), expected);
    }

    /// 指数部を持つ `_` 区切りの浮動小数点にも引用符を付けること
    #[test]
    fn json_to_yaml_quotes_underscore_floats_with_exponent() {
        let input = r#"["685_230.15e+03","1_0.5E-2","1_0e3","1_0.5e3"]"#;
        // 指数に符号が無い、または小数点が無いものは YAML 1.1 でも数値ではない
        let output = json_to_yaml(input);
        assert!(output.contains("- '685_230.15e+03'\n"), "{output}");
        assert!(output.contains("- '1_0.5E-2'\n"), "{output}");
        assert!(output.contains("- 1_0e3\n"), "{output}");
        assert!(output.contains("- 1_0.5e3\n"), "{output}");
    }

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

    /// 先頭に BOM が付いた JSON も整形・変換できるようにすること
    #[test]
    fn test_json_functions_ignore_leading_bom() {
        let input = "\u{FEFF}{\"b\":1,\"a\":2}";
        assert_eq!(format_json(input), "{\n  \"a\": 2,\n  \"b\": 1\n}");
        assert_eq!(
            format_json_preserve_order(input),
            "{\n  \"b\": 1,\n  \"a\": 2\n}"
        );
        assert_eq!(json_to_yaml(input), "a: 2\nb: 1\n");
        assert_eq!(json_to_yaml_preserve_order(input), "b: 1\na: 2\n");
        // BOM だけが違う整形済みの JSON は BOM を除いた文字列にする
        assert_eq!(format_json("\u{FEFF}{}"), "{}");
        // 先頭以外の BOM は JSON として不正なので変換しない
        let inner = "{\"a\":\u{FEFF}1}";
        assert_eq!(format_json(inner), inner);
    }

    /// 同じオブジェクト内のキーが重複した JSON は変換せず元の文字列を返すこと
    #[test]
    fn test_json_functions_reject_duplicate_keys() {
        for input in [
            r#"{"a":1,"a":2}"#,
            r#"{"x":{"b":1,"b":2}}"#,
            r#"[{"a":1,"a":1}]"#,
            r#"{"a":1,"a":2}"#,
        ] {
            assert_eq!(format_json(input), input);
            assert_eq!(format_json_preserve_order(input), input);
            assert_eq!(json_to_yaml(input), input);
            assert_eq!(json_to_yaml_preserve_order(input), input);
        }
    }

    /// パースに失敗した入力は、失敗の理由を返すこと
    #[test]
    fn rejects_with_failure_reason() {
        for (input, expected) in [
            (r#"{"a":1,"b":}"#, ParseFailure::InvalidJson),
            (r#"{"a":1,}"#, ParseFailure::InvalidJson),
            ("", ParseFailure::InvalidJson),
            (r#"{"a":1,"a":2}"#, ParseFailure::DuplicateKey),
            (r#"[{"x":{"b":1,"b":2}}]"#, ParseFailure::DuplicateKey),
        ] {
            assert_eq!(super::format_json(input), Err(expected), "{input:?}");
            assert_eq!(
                super::format_json_preserve_order(input),
                Err(expected),
                "{input:?}"
            );
            assert_eq!(super::json_to_yaml(input), Err(expected), "{input:?}");
            assert_eq!(
                super::json_to_yaml_preserve_order(input),
                Err(expected),
                "{input:?}"
            );
        }
        // YAML で表現できない数値は、パースではなく変換の失敗として扱う
        let out_of_range = r#"{"a":1e400}"#;
        assert_eq!(
            super::json_to_yaml(out_of_range),
            Err(ParseFailure::UnrepresentableValue)
        );
        assert_eq!(
            super::json_to_yaml_preserve_order(out_of_range),
            Err(ParseFailure::UnrepresentableValue)
        );
        // 整形済み・対象外の入力は失敗ではない
        assert!(super::format_json("{}").is_ok());
        assert!(super::json_to_yaml("123").is_ok());
    }

    /// 別々のオブジェクトに同じキーがあっても重複として扱わないこと
    #[test]
    fn test_json_functions_accept_same_key_in_different_objects() {
        let input = r#"{"a":{"a":1},"b":[{"a":2},{"a":3}],"c":1.5e3}"#;
        assert!(matches!(format_json(input), Cow::Owned(_)));
        assert!(matches!(format_json_preserve_order(input), Cow::Owned(_)));
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

    /// YAML 変換で 128 bit 整数を超える整数と 17 桁を超える小数を元の桁のまま出力すること
    #[test]
    fn test_json_to_yaml_keeps_digits_beyond_f64() {
        let input = concat!(
            r#"{"a":123456789012345678901234567890123456789012,"#,
            r#""b":3.141592653589793238462643,"#,
            r#""c":-0.1234567890123456789,"#,
            r#""d":1.00000000000000000001e5,"#,
            r#""e":12345678901234567890123456789012345678901e3,"#,
            r#""f":[1.5,0.1,9007199254740993]}"#
        );
        let expected = "\
a: 123456789012345678901234567890123456789012
b: 3.141592653589793238462643
c: -0.1234567890123456789
d: 1.00000000000000000001e+5
e: 12345678901234567890123456789012345678901.0e+3
f:
- 1.5
- 0.1
- 9007199254740993
";
        assert_eq!(json_to_yaml(input), expected);
        assert_eq!(json_to_yaml_preserve_order(input), expected);
    }

    /// オブジェクトでも配列でもない JSON は YAML へ変換せず元の文字列を返すこと
    #[test]
    fn test_json_to_yaml_keeps_scalar_only_input() {
        for input in [
            "\"plain text\"",
            "123",
            "1.5",
            "true",
            "null",
            " 123 ",
            "\u{FEFF}\"text\"",
        ] {
            assert!(matches!(json_to_yaml(input), Cow::Borrowed(_)), "{input:?}");
            assert!(
                matches!(json_to_yaml_preserve_order(input), Cow::Borrowed(_)),
                "{input:?}"
            );
        }
        // 空のオブジェクト・配列は変換する
        assert_eq!(json_to_yaml("{}"), "{}\n");
        assert_eq!(json_to_yaml_preserve_order("[]"), "[]\n");
    }

    /// 桁を保った数値を YAML から JSON へ戻すと元の JSON と同じ数値になること
    #[test]
    fn test_json_yaml_round_trip_keeps_digits_beyond_f64() {
        use crate::refiner::transform::yaml::yaml_to_json;

        let input =
            r#"{"a":123456789012345678901234567890123456789012,"b":3.141592653589793238462643}"#;
        let yaml = json_to_yaml(input);
        let json = yaml_to_json(&yaml).expect("YAML として読める");
        assert!(
            json.contains("\"a\": 123456789012345678901234567890123456789012"),
            "{json}"
        );
        assert!(json.contains("\"b\": 3.141592653589793238462643"), "{json}");
    }

    /// 数値と同じ形の文字列は桁を保った数値と取り違えず文字列のまま出力すること
    #[test]
    fn test_json_to_yaml_keeps_number_like_strings() {
        let input = r#"{"a":"3.141592653589793238462643","b":3.141592653589793238462643}"#;
        let output = json_to_yaml(input);
        assert_eq!(
            output,
            "a: '3.141592653589793238462643'\nb: 3.141592653589793238462643\n"
        );
    }

    /// YAML 1.1 で真偽値になる文字列は引用符で囲って出力すること
    #[test]
    fn test_json_to_yaml_quotes_yaml11_bool_words() {
        let input = r#"{"a":"yes","b":"No","c":"ON","d":"off","e":"y","f":"N"}"#;
        for output in [json_to_yaml(input), json_to_yaml_preserve_order(input)] {
            for expected in [
                "a: 'yes'
",
                "b: 'No'
",
                "c: 'ON'
",
                "d: 'off'
",
                "e: 'y'
",
                "f: 'N'
",
            ] {
                assert!(output.contains(expected), "{output}");
            }
        }
    }

    /// キーと配列要素の真偽値語も引用符で囲うようにすること
    #[test]
    fn test_json_to_yaml_quotes_yaml11_bool_words_in_keys_and_arrays() {
        let input = r#"{"on":["yes","no"],"k":{"y":"n"}}"#;
        for output in [json_to_yaml(input), json_to_yaml_preserve_order(input)] {
            assert!(output.contains("'on':"), "{output}");
            assert!(output.contains("- 'yes'"), "{output}");
            assert!(output.contains("- 'no'"), "{output}");
            assert!(output.contains("'y': 'n'"), "{output}");
        }
    }

    /// YAML 1.1 で日付・timestamp・整数・60 進数になる文字列は引用符で囲って出力すること
    #[test]
    fn test_json_to_yaml_quotes_yaml11_non_string_forms() {
        let input = r#"{"h":"2024-01-01","t":"2024-01-01T00:00:00Z","l":"1_000","i":"1:30","s":"2024-01-01 00:00:00 +09:00","x":"0x_FF","f":"190:20:30.15"}"#;
        for output in [json_to_yaml(input), json_to_yaml_preserve_order(input)] {
            for expected in [
                "h: '2024-01-01'\n",
                "t: '2024-01-01T00:00:00Z'\n",
                "l: '1_000'\n",
                "i: '1:30'\n",
                "s: '2024-01-01 00:00:00 +09:00'\n",
                "x: '0x_FF'\n",
                "f: '190:20:30.15'\n",
            ] {
                assert!(output.contains(expected), "{output}");
            }
        }
    }

    /// 日付・時刻に似るだけの文字列は引用符を付けないようにすること
    #[test]
    fn test_json_to_yaml_keeps_date_like_strings_plain() {
        let input =
            r#"{"a":"2024-01-01 meeting","b":"12:00pm","c":"v1_000","d":"1:60","e":"2024-01"}"#;
        let output = json_to_yaml(input);
        assert_eq!(
            output,
            "a: 2024-01-01 meeting
b: 12:00pm
c: v1_000
d: 1:60
e: 2024-01
"
        );
    }

    /// 真偽値語を含むだけの文字列や、他の語は引用符を付けないようにすること
    #[test]
    fn test_json_to_yaml_keeps_other_strings_plain() {
        let input = r#"{"a":"yes sir","b":"yesterday","c":"ok"}"#;
        let output = json_to_yaml(input);
        assert_eq!(
            output,
            "a: yes sir
b: yesterday
c: ok
"
        );
    }

    /// 置き換え用の接頭辞を含む入力でも文字列を壊さないようにすること
    #[test]
    fn test_json_to_yaml_survives_prefix_collision() {
        let input = r#"{"a":"__clip_refiner_yaml11_yes__","b":"yes"}"#;
        let output = json_to_yaml(input);
        assert_eq!(
            output,
            "a: __clip_refiner_yaml11_yes__
b: 'yes'
"
        );
    }

    /// YAML で表現できない数値は元の文字列を返すようにすること
    #[test]
    fn test_json_to_yaml_out_of_range_number_returns_input() {
        let input = r#"{"a":1e400}"#;
        assert_eq!(json_to_yaml(input), input);
        assert_eq!(json_to_yaml_preserve_order(input), input);
    }
}
