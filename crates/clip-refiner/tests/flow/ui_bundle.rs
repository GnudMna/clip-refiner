//! コミット済み UI バンドル (`*.bundle.js`) がソースと同期していることのテスト
//!
//! `scripts/common/build-ui.ts` が先頭に刻んだ入力一覧と SHA-256 を Node なしで再計算し、
//! ソースだけ更新してバンドルの再生成を忘れた状態を `cargo test` で検出する

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

// ======================================================================
// 定数
// ======================================================================

/// 検証対象のバンドル (リポジトリルート相対)
const BUNDLES: [&str; 3] = [
    "crates/clip-refiner/src/ui/settings/settings.bundle.js",
    "crates/clip-refiner/src/ui/selector/quick_selector.bundle.js",
    "crates/clip-refiner/src/ui/selector/clip_selector.bundle.js",
];

const HASH_PREFIX: &str = "/* inputs-sha256: ";
const INPUT_PREFIX: &str = "/* input: ";
const COMMENT_SUFFIX: &str = " */";

// ======================================================================
// ヘルパー
// ======================================================================

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// バンドル先頭のヘッダーから記録済みハッシュと入力一覧を取り出す
///
/// # Arguments
/// * `bundle` - バンドルの内容
///
/// # Returns
/// * `Option<(String, Vec<String>)>` - ヘッダーが無い場合は `None`
fn parse_header(bundle: &str) -> Option<(String, Vec<String>)> {
    let mut lines = bundle.lines().skip(1);
    let hash = lines
        .next()?
        .strip_prefix(HASH_PREFIX)?
        .strip_suffix(COMMENT_SUFFIX)?
        .to_string();
    let inputs = lines
        .map_while(|line| {
            line.strip_prefix(INPUT_PREFIX)?
                .strip_suffix(COMMENT_SUFFIX)
        })
        .map(str::to_string)
        .collect();
    Some((hash, inputs))
}

/// 入力ファイル群の SHA-256 を `build-ui.ts` と同じ手順で求める
///
/// パス昇順に `パス \0 内容 \0` を連結してハッシュする
///
/// # Arguments
/// * `inputs` - リポジトリルート相対の `/` 区切りパス
///
/// # Returns
/// * `Result<String, String>` - 小文字 16 進。入力が読めない場合は理由
fn hash_inputs(inputs: &[String]) -> Result<String, String> {
    let mut sorted = inputs.to_vec();
    sorted.sort();
    let mut hasher = Sha256::new();
    for input in &sorted {
        let content = fs::read(repo_root().join(input))
            .map_err(|error| format!("入力 {input} を読めない: {error}"))?;
        hasher.update(input.as_bytes());
        hasher.update([0]);
        hasher.update(&content);
        hasher.update([0]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        }))
}

// ======================================================================
// テスト
// ======================================================================

/// 全バンドルの記録ハッシュが現在のソースから求めた値と一致すること
///
/// 不一致はソースだけ更新してバンドルを再生成していない状態を表す
#[test]
fn committed_ui_bundles_match_sources() {
    let mut stale = Vec::new();
    for bundle in BUNDLES {
        let content = fs::read_to_string(repo_root().join(bundle))
            .unwrap_or_else(|error| panic!("{bundle} を読めない: {error}"));
        let (recorded, inputs) = parse_header(&content)
            .unwrap_or_else(|| panic!("{bundle} に入力ハッシュのヘッダーが無い"));

        // ヘッダーには少なくともエントリとビルドスクリプトが載る
        assert!(
            inputs
                .iter()
                .any(|input| input == "scripts/common/build-ui.ts"),
            "{bundle} の入力一覧に build-ui.ts が無い"
        );

        match hash_inputs(&inputs) {
            Ok(actual) if actual == recorded => {}
            Ok(_) => stale.push(format!("{bundle} (ソース変更あり)")),
            Err(reason) => stale.push(format!("{bundle} ({reason})")),
        }
    }

    assert!(
        stale.is_empty(),
        "UI バンドルがソースと同期していない。`npm run build:ui` で再生成してコミットすること:\n  {}",
        stale.join("\n  ")
    );
}

/// ヘッダーから記録ハッシュと入力一覧を取り出せること
#[test]
fn parse_header_reads_hash_and_inputs() {
    let bundle = "/* Generated */\n/* inputs-sha256: abc */\n/* input: a.ts */\n/* input: b.ts */\n(() => {\n";
    let (hash, inputs) = parse_header(bundle).expect("ヘッダーを解析できる");

    assert_eq!(hash, "abc");
    assert_eq!(inputs, ["a.ts", "b.ts"]);

    // ヘッダーが無い旧形式は `None`
    assert!(parse_header("/* Generated */\n(() => {\n").is_none());
}
