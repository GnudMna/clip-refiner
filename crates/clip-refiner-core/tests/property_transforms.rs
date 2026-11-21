//! 加工変換のプロパティベーステスト
//!
//! 代表入力の回帰 (`refine_modes.rs`) では拾えない境界ケースを、任意入力で成り立つ性質から検出する
//! 性質は 件数・内容の保存 / 往復 / 冪等性 (`f(f(x)) == f(x)`) / パニックしないこと の 4 種

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "テスト専用のため、ヘルパー内の失敗もそのまま panic させる"
)]

use std::borrow::Cow;
use std::collections::BTreeMap;

use clip_refiner_core::config::RegexSettings;
use clip_refiner_core::{RefineContext, RefineMode, Refiner};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use serde_json::Value;
use strum::IntoEnumIterator;

// ======================================================================
// 設定
// ======================================================================
/// 失敗した入力の保存先を `tests/property_transforms.proptest-regressions` に固定した設定
///
/// 統合テストには `lib.rs` / `main.rs` が無く、既定の保存先を解決できず警告が出るため明示する
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(
            "proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

// ======================================================================
// 入力生成
// ======================================================================
/// 改行を含まない 1 行分の文字列
///
/// CSV のクォートや区切り、Markdown の表記号、全角文字を混ぜて境界を踏みやすくする
fn line() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 ,\"|\t-]{0,12}",
        "[^\r\n]{0,12}",
        // 行中の単独の CR (行末の CR は次の LF と CRLF になるため置かない)
        "[a-z,\"\r]{0,8}".prop_map(|s| format!("{s}x")),
    ]
}

/// 改行コード (LF / CRLF) をそろえた複数行テキスト
fn lines_text() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(line(), 0..12),
        prop_oneof![Just("\n"), Just("\r\n")],
        any::<bool>(),
    )
        .prop_map(|(lines, ending, trailing)| {
            let mut text = lines.join(ending);
            if trailing {
                text.push_str(ending);
            }
            text
        })
}

/// クォート内改行を含むレコードが混ざる CSV テキスト
///
/// 並べ替えのレコード単位の経路 (`split_csv_records`) を通すための入力
fn multiline_csv_text() -> impl Strategy<Value = String> {
    let cell = prop_oneof![
        "[a-zA-Z0-9 ]{0,6}",
        "[a-z0-9 \n,]{0,8}".prop_map(|s| format!("\"{s}\"")),
    ];
    (
        1..4usize,
        prop::collection::vec(prop::collection::vec(cell, 4), 1..8),
        prop_oneof![Just("\n"), Just("\r\n")],
    )
        .prop_map(|(cols, rows, ending)| {
            rows.iter()
                .map(|row| row[..=cols].join(","))
                .collect::<Vec<_>>()
                .join(ending)
                .replace('\n', ending)
                .replace("\r\r\n", "\r\n")
        })
}

/// 表のセル (前後の空白なし。Markdown の表記号と空セルを含む)
fn table_cell() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 |:\"-]{0,6}".prop_map(|s| s.trim().to_string())
}

/// 列数のそろった表 (1 行目が見出し)
fn table() -> impl Strategy<Value = Vec<Vec<String>>> {
    (2..5usize).prop_flat_map(|cols| {
        prop::collection::vec(prop::collection::vec(table_cell(), cols), 1..8)
    })
}

/// 識別子らしい複数行テキスト
fn identifier_text() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-zA-Z0-9_ -]{0,16}", 0..4).prop_map(|lines| lines.join("\n"))
}

/// 単語を区切り文字 (`_` / `-` / 空白) と camelCase の境界を混ぜてつないだ識別子
///
/// 単語は英小文字 2 文字以上に限る。1 文字の単語や数字だけの単語は camelCase にすると
/// 境界が消える (`a_b` → `aB` → `ab`) ため、往復の性質を検証する対象から外す
fn mixed_identifier() -> impl Strategy<Value = String> {
    let word = ("[a-z]{2,6}", any::<bool>());
    let separator = prop_oneof![Just(""), Just("_"), Just("-"), Just(" ")];
    (word, prop::collection::vec((separator, word), 0..4)).prop_map(|(first, rest)| {
        let capitalize = |word: &str| format!("{}{}", word[..1].to_uppercase(), &word[1..]);
        let mut text = if first.1 {
            capitalize(&first.0)
        } else {
            first.0
        };
        for (separator, (word, upper)) in rest {
            text.push_str(separator);
            // 区切り文字なしでつなぐ単語は先頭を大文字にして camelCase の境界にする
            if separator.is_empty() || upper {
                text.push_str(&capitalize(&word));
            } else {
                text.push_str(&word);
            }
        }
        text
    })
}

/// JSON の数値 (`u128` / `i128` を超えない整数と有限の浮動小数点)
fn json_number() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<i64>().prop_map(|n| n.to_string()),
        any::<u128>().prop_map(|n| n.to_string()),
        any::<i128>().prop_map(|n| n.to_string()),
        any::<f64>()
            .prop_filter("有限値のみ", |f| f.is_finite())
            .prop_map(|f| format!("{f:?}")),
    ]
}

/// 任意の JSON 値
fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        json_number()
            .prop_map(|n| serde_json::from_str(&n).expect("生成した数値は JSON として有効")),
        any::<String>().prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map(any::<String>(), inner, 0..6)
                .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

/// 全モードに与える任意文字列
///
/// 完全な任意文字列に加え、JSON / YAML / CSV / Markdown / HTML の構文記号を高密度に含む文字列を混ぜる
fn any_text() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        "[\\[\\]{}<>/|!\\-=:,.#*_`'\"\\\\&%+~ \t\r\na-zA-Z0-9ａあｱﾞ]{0,64}",
    ]
}

// ======================================================================
// ヘルパー
// ======================================================================
/// 既定のコンテキストでモードを適用する
fn refine(mode: RefineMode, text: &str) -> String {
    mode.refine(text, &RefineContext::default()).into_owned()
}

/// 行の出現回数 (行の多重集合)
fn line_counts(text: &str) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        *counts.entry(line).or_insert(0) += 1;
    }
    counts
}

/// 表を Markdown の表へ組み立てる (`|` はエスケープする)
fn to_markdown_table(rows: &[Vec<String>]) -> String {
    let render = |row: &Vec<String>| {
        let cells: Vec<String> = row.iter().map(|cell| cell.replace('|', "\\|")).collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut lines = vec![render(&rows[0])];
    lines.push(format!("|{}", "---|".repeat(rows[0].len())));
    lines.extend(rows[1..].iter().map(render));
    lines.join("\n")
}

/// 表を TSV へ組み立てる (Excel と同じくクォートが必要なセルはクォートする)
fn to_tsv(rows: &[Vec<String>]) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(Vec::new());
    for row in rows {
        writer
            .write_record(row)
            .expect("メモリへの書き込みは失敗しない");
    }
    let bytes = writer.into_inner().expect("メモリへの書き込みは失敗しない");
    String::from_utf8(bytes)
        .expect("UTF-8 のセルから組み立てた TSV")
        .trim_end_matches('\n')
        .to_string()
}

/// TSV を表として読み取る
fn parse_tsv(text: &str) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes())
        .records()
        .map(|record| {
            record
                .expect("変換結果の TSV は解析できる")
                .iter()
                .map(str::to_string)
                .collect()
        })
        .collect()
}

/// JSON 値を数値の表記ゆれ (`1.0` / `1e0` など) を除いて比較できる形にする
///
/// 整数は桁を保ったまま、それ以外の数値は `f64` として比較する
fn normalize_json(value: &Value) -> Value {
    match value {
        Value::Number(n) => {
            let text = n.as_str();
            let canonical = text
                .parse::<i128>()
                .map(|v| v.to_string())
                .or_else(|_| text.parse::<u128>().map(|v| v.to_string()))
                .unwrap_or_else(|_| {
                    let v = n.as_f64().expect("有限の数値");
                    if v == 0.0 {
                        "0.0".to_string()
                    } else {
                        format!("{v:e}")
                    }
                });
            Value::String(format!("number:{canonical}"))
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize_json).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), normalize_json(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

// ======================================================================
// 件数・内容の保存
// ======================================================================
proptest! {
    #![proptest_config(config())]

    /// 行の並べ替えは行の並び順だけを変え、行数と行の集合を保つこと
    #[test]
    fn sort_lines_preserves_line_multiset(input in prop_oneof![lines_text(), multiline_csv_text()]) {
        for mode in [
            RefineMode::SortLinesAsc,
            RefineMode::SortLinesDesc,
            RefineMode::SortLinesNaturalAsc,
            RefineMode::SortLinesNaturalDesc,
        ] {
            let output = refine(mode, &input);
            prop_assert_eq!(line_counts(&output), line_counts(&input), "mode={:?}", mode);
        }
    }

    /// Markdown 表→Excel は見出しとデータ行をすべて残し、セルの内容を保つこと
    #[test]
    fn markdown_table_to_excel_preserves_rows(rows in table()) {
        let markdown = to_markdown_table(&rows);
        let tsv = refine(RefineMode::MarkdownToExcel, &markdown);
        prop_assert_eq!(parse_tsv(&tsv), rows, "markdown={:?}\ntsv={:?}", markdown, tsv);
    }

    /// Excel→Markdown 表は行数 (見出し + 区切り + データ行) を保つこと
    #[test]
    fn excel_to_markdown_preserves_rows(rows in table()) {
        let tsv = to_tsv(&rows);
        let markdown = refine(RefineMode::ExcelToMarkdown, &tsv);
        prop_assert_eq!(markdown.lines().count(), rows.len() + 1, "tsv={:?}\nmarkdown={:?}", tsv, markdown);
    }

    /// 重複行削除は行の集合を保ち、クォートを含まない入力では各行を 1 回ずつ残すこと
    ///
    /// クォート内に改行を含む CSV はレコード単位で比べるため、レコード内の同じ行 (空行など) は重複として削除しない
    #[test]
    fn remove_duplicate_lines_keeps_each_line_once(input in lines_text()) {
        let output = refine(RefineMode::RemoveDuplicateLines, &input);
        let counts = line_counts(&input);
        if input.contains('"') {
            // 削除されるのは重複したレコードだけで、1 件目が残るため行の種類は変わらない
            let kept: Vec<&str> = line_counts(&output).into_keys().collect();
            let original: Vec<&str> = counts.into_keys().collect();
            prop_assert_eq!(kept, original);
        } else {
            let expected: BTreeMap<&str, usize> = counts.into_keys().map(|line| (line, 1)).collect();
            prop_assert_eq!(line_counts(&output), expected);
        }
    }

    /// 空行削除は空白だけの行以外を順序どおりに残すこと
    #[test]
    fn remove_empty_lines_keeps_non_blank_lines(input in lines_text()) {
        let output = refine(RefineMode::RemoveEmptyLines, &input);
        let expected: Vec<&str> = input.lines().filter(|line| !line.trim().is_empty()).collect();
        prop_assert_eq!(output.lines().collect::<Vec<_>>(), expected);
    }
}

/// クォート内の空行を含む CSV レコードは、行単位の重複として削除しないこと
#[test]
fn remove_duplicate_lines_keeps_blank_lines_inside_quoted_field() {
    let input = ",\"x


\"";
    assert_eq!(refine(RefineMode::RemoveDuplicateLines, input), input);
}

// ======================================================================
// 往復
// ======================================================================
proptest! {
    #![proptest_config(config())]

    /// URL エンコード後にデコードすると元の文字列に戻るようにすること
    #[test]
    fn url_encode_decode_roundtrip(input in "\\PC*") {
        let ctx = RefineContext::default();
        let encoded = RefineMode::UrlEncode.refine(&input, &ctx);
        let decoded = RefineMode::UrlDecode.refine(&encoded, &ctx);
        prop_assert_eq!(&decoded, &input);
    }

    /// Base64 エンコード後にデコードすると元の文字列に戻るようにすること
    #[test]
    fn base64_encode_decode_roundtrip(input in "\\PC+") {
        let ctx = RefineContext::default();
        let encoded = RefineMode::Base64Encode.refine(&input, &ctx);
        let decoded = RefineMode::Base64Decode.refine(&encoded, &ctx);
        prop_assert_eq!(&decoded, &input);
    }

    /// JSON エスケープ後にアンエスケープすると元の文字列に戻るようにすること
    #[test]
    fn escape_unescape_roundtrip(input in "[\\x20-\\x7E]*") {
        let ctx = RefineContext::default();
        let escaped = RefineMode::Escape.refine(&input, &ctx);
        let unescaped = RefineMode::Unescape.refine(&escaped, &ctx);
        prop_assert_eq!(&unescaped, &input);
    }

    /// 正規表現エスケープ後にアンエスケープすると元の文字列に戻るようにすること
    #[test]
    fn regex_escape_unescape_roundtrip(input in "[\\x20-\\x7E]*") {
        let ctx = RefineContext::default();
        let escaped = RefineMode::RegexEscape.refine(&input, &ctx);
        let unescaped = RefineMode::RegexUnescape.refine(&escaped, &ctx);
        prop_assert_eq!(&unescaped, &input);
    }

    /// 大きな整数・浮動小数点・任意のキーを含む JSON で、JSON→YAML→JSON の往復後も値が一致すること
    #[test]
    fn json_yaml_json_roundtrip(value in json_value()) {
        // JSON→YAML・YAML→JSON は最上位がオブジェクト・配列でない入力を変換しないため、配列で包む
        let value = if value.is_array() || value.is_object() {
            value
        } else {
            Value::Array(vec![value])
        };
        let json = serde_json::to_string(&value).expect("JSON 値は直列化できる");
        for (to_yaml, to_json) in [
            (RefineMode::JsonToYaml, RefineMode::YamlToJson),
            (RefineMode::JsonToYamlPreserveOrder, RefineMode::YamlToJsonPreserveOrder),
        ] {
            let ctx = RefineContext::default();
            let yaml = to_yaml.refine(&json, &ctx);
            prop_assert!(matches!(yaml, Cow::Owned(_)), "YAML へ変換できない: {}", json);
            let back = to_json.refine(&yaml, &ctx);
            prop_assert!(matches!(back, Cow::Owned(_)), "JSON へ戻せない: {}", yaml);
            let back: Value = serde_json::from_str(&back).expect("変換結果は JSON");
            prop_assert_eq!(normalize_json(&back), normalize_json(&value), "mode={:?}\nyaml={}", to_yaml, yaml);
        }
    }

    /// JSON 整形は値を変えないこと
    ///
    /// 数値の表記も保つ
    #[test]
    fn json_format_preserves_value(value in json_value()) {
        let json = serde_json::to_string(&value).expect("JSON 値は直列化できる");
        for mode in [RefineMode::JsonFormat, RefineMode::JsonFormatPreserveOrder] {
            let formatted: Value = serde_json::from_str(&refine(mode, &json)).expect("整形結果は JSON");
            prop_assert_eq!(&formatted, &value, "mode={:?}", mode);
        }
    }

    /// 半角→全角→半角で元の文字列に戻ること
    #[test]
    fn halfwidth_fullwidth_roundtrip(input in "[\\x20-\\x7E\u{FF61}-\u{FF9F}]{0,32}") {
        let full = refine(RefineMode::ToFullwidth, &input);
        prop_assert_eq!(refine(RefineMode::ToHalfwidth, &full), input);
    }

    /// 全角→半角→全角で元の文字列に戻ること
    ///
    /// 全角側は半角から変換できる文字 (半角入力を全角にしたもの) に限る
    #[test]
    fn fullwidth_halfwidth_roundtrip(seed in "[\\x20-\\x7E\u{FF61}-\u{FF9F}]{0,32}") {
        let input = refine(RefineMode::ToFullwidth, &seed);
        let half = refine(RefineMode::ToHalfwidth, &input);
        prop_assert_eq!(refine(RefineMode::ToFullwidth, &half), input);
    }

    /// TSV→Markdown 表→TSV でセルの内容が一致すること
    #[test]
    fn tsv_markdown_tsv_roundtrip(rows in table()) {
        let tsv = to_tsv(&rows);
        let markdown = refine(RefineMode::ExcelToMarkdown, &tsv);
        let back = refine(RefineMode::MarkdownToExcel, &markdown);
        prop_assert_eq!(parse_tsv(&back), rows, "markdown={:?}", markdown);
    }

    /// Markdown 表→TSV→Markdown 表で表が一致すること
    #[test]
    fn markdown_tsv_markdown_roundtrip(rows in table()) {
        let markdown = to_markdown_table(&rows);
        let tsv = refine(RefineMode::MarkdownToExcel, &markdown);
        let back = refine(RefineMode::ExcelToMarkdown, &tsv);
        // 空セルの空白など表記ゆれを除くため、もう一度 TSV にして比較する
        prop_assert_eq!(parse_tsv(&refine(RefineMode::MarkdownToExcel, &back)), rows, "tsv={:?}\nback={:?}", tsv, back);
    }
}

// ======================================================================
// 冪等性
// ======================================================================
/// 2 回適用しても結果が変わらないモード
const IDEMPOTENT_MODES: &[RefineMode] = &[
    RefineMode::Trim,
    RefineMode::TrimLines,
    RefineMode::RemoveEmptyLines,
    RefineMode::RemoveDuplicateLines,
    RefineMode::SortLinesAsc,
    RefineMode::SortLinesDesc,
    RefineMode::SortLinesNaturalAsc,
    RefineMode::SortLinesNaturalDesc,
    RefineMode::JsonFormat,
    RefineMode::JsonFormatPreserveOrder,
    RefineMode::ToHalfwidth,
    RefineMode::ToFullwidth,
    RefineMode::PathToSlash,
    RefineMode::PathToBackslash,
    RefineMode::RemoveComma,
];

/// 単語を区切り文字でつなぐケース変換モード
///
/// 区切り文字で単語の境界が残るため、任意の入力で冪等になる
const SEPARATED_CASE_MODES: &[RefineMode] = &[
    RefineMode::ToSnakeCase,
    RefineMode::ToKebabCase,
    RefineMode::ToScreamingSnakeCase,
];

/// ケース変換モード
const CASE_MODES: &[RefineMode] = &[
    RefineMode::ToCamelCase,
    RefineMode::ToSnakeCase,
    RefineMode::ToPascalCase,
    RefineMode::ToKebabCase,
    RefineMode::ToScreamingSnakeCase,
];

proptest! {
    #![proptest_config(config())]

    /// 冪等なモードは 2 回適用しても結果が変わらないこと
    ///
    /// 末尾が単独の CR のテキストは除く。行単位の処理で最終行の CR が次の行の LF と
    /// 結合して CRLF になる (旧 Mac 形式の改行で、クリップボードではまず現れない)
    #[test]
    fn modes_are_idempotent(
        input in prop_oneof![lines_text(), any_text(), multiline_csv_text()]
            .prop_filter("末尾が単独の CR", |text| !text.ends_with('\r'))
    ) {
        for &mode in IDEMPOTENT_MODES {
            let once = refine(mode, &input);
            prop_assert_eq!(refine(mode, &once), once, "mode={:?}", mode);
        }
    }

    /// JSON 整形は整形済みの JSON を変えないこと
    #[test]
    fn json_format_is_idempotent(value in json_value()) {
        let json = serde_json::to_string(&value).expect("JSON 値は直列化できる");
        for mode in [RefineMode::JsonFormat, RefineMode::JsonFormatPreserveOrder] {
            let once = refine(mode, &json);
            prop_assert_eq!(refine(mode, &once), once, "mode={:?}", mode);
        }
    }

    /// 区切り文字を使うケース変換は 2 回適用しても結果が変わらないこと
    ///
    /// 大文字を持たない小文字 (`º` など) は大文字化しても小文字のままで、次の変換で
    /// camelCase の境界とみなされるため、入力は英数字・全角英字・かな漢字に限る
    #[test]
    fn separated_case_conversion_is_idempotent(
        input in prop_oneof![identifier_text(), "[a-zA-Z0-9_ \t\nａｚＡＺあア漢-]{0,24}"]
    ) {
        for &mode in SEPARATED_CASE_MODES {
            let once = refine(mode, &input);
            prop_assert_eq!(refine(mode, &once), once, "mode={:?}", mode);
        }
    }

    /// ケース変換は識別子に 2 回適用しても結果が変わらないこと
    #[test]
    fn case_conversion_is_idempotent(input in mixed_identifier()) {
        for &mode in CASE_MODES {
            let once = refine(mode, &input);
            prop_assert_eq!(refine(mode, &once), once, "mode={:?}", mode);
        }
    }

    /// ケース変換の結果を別のケースへ変換しても単語の区切りを保つこと
    ///
    /// どのケースを経由しても、最後に同じケースへ変換すれば同じ結果になる
    #[test]
    fn case_conversion_preserves_words(input in mixed_identifier()) {
        for &via in CASE_MODES {
            let intermediate = refine(via, &input);
            for &target in CASE_MODES {
                prop_assert_eq!(
                    refine(target, &intermediate),
                    refine(target, &refine(RefineMode::ToSnakeCase, &input)),
                    "via={:?} target={:?}", via, target
                );
            }
        }
    }

    /// 数字のみの文字列は正規表現置換で一貫して変換されるようにすること
    #[test]
    fn regex_replace_digits_are_stable(input in "[0-9]{0,32}") {
        let ctx = RefineContext::with_regex(RegexSettings::single(r"\d", "X"));
        let once = RefineMode::RegexReplace.refine(&input, &ctx);
        let twice = RefineMode::RegexReplace.refine(&once, &ctx);
        prop_assert_eq!(&once, &twice);
    }
}

// ======================================================================
// パニックしないこと
// ======================================================================
proptest! {
    #![proptest_config(config())]

    /// 処理上限内の任意の文字列に対し、全モードがパニックせず結果を返すこと
    #[test]
    fn all_modes_do_not_panic(input in any_text()) {
        let contexts = [
            RefineContext::default(),
            RefineContext::with_regex(RegexSettings::single(r"(\w+)\s*", "[$1]")),
        ];
        for ctx in &contexts {
            for mode in RefineMode::iter() {
                let _ = mode.refine(&input, ctx);
            }
        }
    }

    /// 生成した JSON / Markdown 表 / CSV を全モードに与えてもパニックしないこと
    ///
    /// 構文として有効な入力でパーサの奥まで到達させる
    #[test]
    fn all_modes_do_not_panic_on_structured_input(
        input in prop_oneof![
            json_value().prop_map(|v| serde_json::to_string_pretty(&v).expect("JSON 値は直列化できる")),
            table().prop_map(|rows| to_markdown_table(&rows)),
            table().prop_map(|rows| to_tsv(&rows)),
            multiline_csv_text(),
        ]
    ) {
        let ctx = RefineContext::default();
        for mode in RefineMode::iter() {
            let _ = mode.refine(&input, &ctx);
        }
    }
}
