//! パーサ系・行操作・文字変換・正規表現変換の処理時間ベンチマーク
//!
//! 巨大入力 (パーサ入力上限 `MAX_PARSER_INPUT_BYTES` の近傍) で処理時間が悪化していないかを、
//! 保存した基準 (baseline) との比較で検知する。実行は `./scripts/windows/bench.ps1`
//!
//! 入力が上限超過や変換失敗で素通りすると、何も測らないまま常に速く見えてしまう
//! そのため計測前に、各入力が実際に加工されることを検証する

use std::borrow::Cow;
use std::fmt::Write as _;
use std::hint::black_box;

use clip_refiner_core::{RefineContext, RefineMode, Refiner, RegexSettings};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

// ======================================================================
// 定数
// ======================================================================
/// 計測する入力サイズ (バイト)
///
/// 大きい方はパーサ入力上限 (1 MiB) の手前に収め、上限超過による素通りを避ける
const INPUT_SIZES: [usize; 2] = [64 * 1024, 960 * 1024];

/// 目標バイト長から入力文字列を作る関数
type InputGenerator = fn(usize) -> String;

// ======================================================================
// 入力生成
// ======================================================================
/// 指定サイズ以上になるまで `row` の出力を連結して文字列を作る
///
/// # Arguments
/// * `target` - 目標バイト長
/// * `prefix` - 先頭に置く文字列
/// * `row` - 行番号から 1 行分を書き出す関数
///
/// # Returns
/// * `String` - `target` バイト以上で、`target` を大きく超えない文字列
fn build_text(target: usize, prefix: &str, mut row: impl FnMut(&mut String, usize)) -> String {
    let mut text = String::with_capacity(target + 256);
    text.push_str(prefix);
    let mut index = 0;
    while text.len() < target {
        row(&mut text, index);
        index += 1;
    }
    text
}

/// オブジェクトの配列からなる JSON を作る
///
/// 入れ子・配列・数値・日本語・エスケープ文字を含める
fn json_input(target: usize) -> String {
    let mut text = build_text(target, "[", |out, i| {
        if i > 0 {
            out.push(',');
        }
        // `write!` は `String` への書き込みで失敗しない
        let _ = write!(
            out,
            r#"{{"id":{i},"name":"項目{i}","price":{}.5,"tags":["a","b\n\"c\"","d"],"nested":{{"flag":{},"note":null,"path":"C:\\dir\\file{i}.txt"}}}}"#,
            i * 7,
            i % 2 == 0,
        );
    });
    text.push(']');
    text
}

/// 入れ子のマップとシーケンスからなる YAML を作る
fn yaml_input(target: usize) -> String {
    build_text(target, "", |out, i| {
        let _ = write!(
            out,
            "- id: {i}\n  name: 項目{i}\n  price: {}.5\n  tags:\n    - a\n    - 'b c'\n  nested:\n    flag: {}\n    note: null\n    path: 'C:\\dir\\file{i}.txt'\n",
            i * 7,
            i % 2 == 0,
        );
    })
}

/// 見出し・段落・リスト・コード・表・リンクを含む Markdown を作る
fn markdown_input(target: usize) -> String {
    build_text(target, "", |out, i| {
        let _ = write!(
            out,
            "## 見出し {i}\n\n段落 {i} には **強調** と *斜体* と `code` と [リンク](https://example.com/{i}?q=1) を含む。\n\n- 項目 A{i}\n- 項目 B{i}\n  - 入れ子 {i}\n\n1. 手順 1\n2. 手順 2\n\n```rust\nlet x = {i};\n```\n\n| 列1 | 列2 | 列3 |\n| --- | --- | --- |\n| a{i} | b{i} | c{i} |\n\n",
        );
    })
}

/// Markdown 表だけからなる文字列を作る (Markdown 表→Excel 用)
fn markdown_table_input(target: usize) -> String {
    build_text(
        target,
        "| 列1 | 列2 | 列3 |\n| --- | --- | --- |\n",
        |out, i| {
            let _ = writeln!(out, "| a{i} | b{i} | c{i} |");
        },
    )
}

/// 入れ子のある HTML を作る (HTML→Markdown 用)
fn html_input(target: usize) -> String {
    build_text(target, "<div>", |out, i| {
        let _ = write!(
            out,
            "<h2>見出し {i}</h2><p>段落 <strong>強調</strong> <a href=\"https://example.com/{i}\">リンク</a></p><ul><li>A{i}</li><li>B{i}</li></ul><table><tr><td>a{i}</td><td>b{i}</td></tr></table>",
        );
    }) + "</div>"
}

/// 逆順に近い行を並べた文字列を作る (行ソート用)
fn lines_input(target: usize) -> String {
    build_text(target, "", |out, i| {
        let _ = writeln!(out, "line-{:08}-{}", 99_999_999 - i, i % 97);
    })
}

/// 数値・英単語・空白が混在する文字列を作る (正規表現置換用)
fn words_input(target: usize) -> String {
    build_text(target, "", |out, i| {
        let _ = writeln!(out, "word{i} 12345 foo_bar  baz   qux {i}");
    })
}

// ======================================================================
// ベンチ本体
// ======================================================================
/// 1 つのモードについて、入力サイズ別の処理時間を計測する
///
/// 計測前に、入力が上限で弾かれず実際に加工されることを検証する
///
/// # Arguments
/// * `c` - Criterion 本体
/// * `group_name` - ベンチグループ名 (モード名)
/// * `mode` - 計測する変換モード
/// * `ctx` - 変換コンテキスト
/// * `make_input` - サイズから入力を作る関数
fn bench_mode(
    c: &mut Criterion,
    group_name: &str,
    mode: RefineMode,
    ctx: &RefineContext,
    make_input: InputGenerator,
) {
    let mut group = c.benchmark_group(group_name);
    for size in INPUT_SIZES {
        let input = make_input(size);
        assert!(
            matches!(mode.refine(&input, ctx), Cow::Owned(_)),
            "{group_name} ({size} バイト) の入力が加工されない。入力生成か上限を見直すこと"
        );
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| mode.refine(black_box(input), ctx));
        });
    }
    group.finish();
}

fn bench_transforms(c: &mut Criterion) {
    let ctx = RefineContext::default();
    let cases: [(&str, RefineMode, InputGenerator); 10] = [
        ("json_format", RefineMode::JsonFormat, json_input),
        (
            "json_format_preserve_order",
            RefineMode::JsonFormatPreserveOrder,
            json_input,
        ),
        ("json_to_yaml", RefineMode::JsonToYaml, json_input),
        ("yaml_to_json", RefineMode::YamlToJson, yaml_input),
        (
            "markdown_to_html",
            RefineMode::MarkdownToHtml,
            markdown_input,
        ),
        (
            "markdown_to_excel",
            RefineMode::MarkdownToExcel,
            markdown_table_input,
        ),
        ("html_to_markdown", RefineMode::HtmlToMarkdown, html_input),
        ("sort_lines_asc", RefineMode::SortLinesAsc, lines_input),
        ("url_encode", RefineMode::UrlEncode, markdown_input),
        ("to_fullwidth", RefineMode::ToFullwidth, markdown_input),
    ];
    for (name, mode, make_input) in cases {
        bench_mode(c, name, mode, &ctx, make_input);
    }

    // 正規表現はマッチタイムアウト付きの別スレッド実行を通るため、他のモードと分けて計測する
    let regex_ctx = RefineContext::with_regex(RegexSettings::single(r"(\w+)\s+(\d+)", "$2-$1"));
    bench_mode(
        c,
        "regex_replace",
        RefineMode::RegexReplace,
        &regex_ctx,
        words_input,
    );
}

criterion_group!(benches, bench_transforms);
criterion_main!(benches);
