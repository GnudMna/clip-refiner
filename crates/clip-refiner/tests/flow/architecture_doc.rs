//! `docs/dev/architecture.md` に書いたパスが実在することのテスト
//!
//! ファイルの移動・改名・削除で構成図や責務表が古くなった状態を `cargo test` で検出する
//! 対象は構成図 (最初のコードブロック)・1 列目の見出しに基準ディレクトリを持つ表・
//! 本文と表でバッククォートに囲んだパス

use std::fs;
use std::path::{Path, PathBuf};

// ======================================================================
// 定数
// ======================================================================

/// 検証対象のドキュメント (リポジトリルート相対)
const DOC: &str = "docs/dev/architecture.md";

/// パスとして扱うバッククォート内トークンの拡張子
const PATH_EXTENSIONS: [&str; 9] = [
    ".rs", ".js", ".ts", ".css", ".html", ".ps1", ".toml", ".md", ".pub",
];

/// リポジトリ外 (実行時に作られる) のため検証しないパス
const RUNTIME_PATHS: [&str; 1] = ["logs/"];

/// 走査しないディレクトリ名 (ビルド成果物・依存)
const SKIPPED_DIRS: [&str; 6] = [
    ".git",
    "target",
    "node_modules",
    "test-results",
    "playwright-report",
    "corpus",
];

// ======================================================================
// ヘルパー
// ======================================================================

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// リポジトリ内の全パスを集める
///
/// # Arguments
/// * `root` - 走査の起点
///
/// # Returns
/// * `Vec<String>` - ルート相対の `/` 区切りパス。ディレクトリは末尾に `/` を付ける
fn collect_repo_paths(root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let entries = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{} を読めない: {error}", dir.display()));
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                if SKIPPED_DIRS.contains(&name.as_str()) {
                    continue;
                }
                let relative = format!("{prefix}{name}/");
                paths.push(relative.clone());
                pending.push((entry.path(), relative));
            } else {
                paths.push(format!("{prefix}{name}"));
            }
        }
    }
    paths
}

/// `*` (区切り `/` をまたがない任意の文字列) を含むパターンでパス全体を照合する
///
/// # Arguments
/// * `pattern` - 照合パターン
/// * `path` - 照合対象のパス
///
/// # Returns
/// * `bool` - 一致した場合は `true`
fn wildcard_match(pattern: &str, path: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == path,
        Some((head, tail)) => {
            let Some(rest) = path.strip_prefix(head) else {
                return false;
            };
            // `*` が食う長さを 0 から順に試す。`/` に達したら打ち切る
            for (index, ch) in rest.char_indices() {
                if wildcard_match(tail, &rest[index..]) {
                    return true;
                }
                if ch == '/' {
                    return false;
                }
            }
            wildcard_match(tail, "")
        }
    }
}

/// `{a,b}` を 1 か所だけ展開する
///
/// # Arguments
/// * `token` - 展開前のパス
///
/// # Returns
/// * `Vec<String>` - 展開後のパス。波括弧が無ければ `token` のみ
fn expand_braces(token: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (token.find('{'), token.find('}')) else {
        return vec![token.to_string()];
    };
    token[open + 1..close]
        .split(',')
        .map(|choice| format!("{}{choice}{}", &token[..open], &token[close + 1..]))
        .collect()
}

/// パス境界で切った末尾がパターンに一致するパスがあるか
///
/// `menu/` は `crates/clip-refiner/src/tray/menu/` に一致する
///
/// # Arguments
/// * `pattern` - 末尾として照合するパターン
/// * `paths` - リポジトリ内の全パス
///
/// # Returns
/// * `bool` - 一致するパスがある場合は `true`
fn exists_as_suffix(pattern: &str, paths: &[String]) -> bool {
    paths.iter().any(|path| {
        let body = path.strip_suffix('/').unwrap_or(path);
        wildcard_match(pattern, path)
            || body
                .match_indices('/')
                .any(|(index, _)| wildcard_match(pattern, &path[index + 1..]))
    })
}

/// バッククォートで囲んだトークンがパスとして検証対象か
///
/// # Arguments
/// * `token` - バッククォート内の文字列
///
/// # Returns
/// * `bool` - `/` で終わるか既知の拡張子を持ち、空白や `:` などを含まない場合は `true`
fn is_path_token(token: &str) -> bool {
    let plain = token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-*{},".contains(ch));
    plain
        && !token.starts_with('-')
        && (token.ends_with('/') || PATH_EXTENSIONS.iter().any(|ext| token.ends_with(ext)))
}

/// 文字列からバッククォートで囲んだトークンを取り出す
///
/// # Arguments
/// * `text` - 対象の文字列
///
/// # Returns
/// * `Vec<&str>` - 出現順のトークン
fn backtick_tokens(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// フェンスで囲んだコードブロックを除いた行と、最初のコードブロックの行に分ける
///
/// # Arguments
/// * `doc` - ドキュメント全体
///
/// # Returns
/// * `(Vec<&str>, Vec<&str>)` - (本文の行, 最初のコードブロックの行)
fn split_code_blocks(doc: &str) -> (Vec<&str>, Vec<&str>) {
    let mut prose = Vec::new();
    let mut first_block = Vec::new();
    let mut in_block = false;
    let mut blocks_seen = 0;
    for line in doc.lines() {
        if line.starts_with("```") {
            in_block = !in_block;
            if in_block {
                blocks_seen += 1;
            }
            continue;
        }
        if !in_block {
            prose.push(line);
        } else if blocks_seen == 1 {
            first_block.push(line);
        }
    }
    (prose, first_block)
}

/// 構成図 (`├──` / `└──` のツリー) からルート相対のパスを取り出す
///
/// 1 行目はルート自身として読み飛ばし、`...` の行と `#` 以降のコメントは無視する
///
/// # Arguments
/// * `lines` - 構成図の行
///
/// # Returns
/// * `Vec<String>` - ルート相対のパス (ディレクトリは末尾 `/`)
fn parse_tree(lines: &[&str]) -> Vec<String> {
    let mut stack: Vec<String> = Vec::new();
    let mut paths = Vec::new();
    for line in lines.iter().skip(1) {
        let Some(connector) = line.find("├── ").or_else(|| line.find("└── ")) else {
            continue;
        };
        // 1 階層は `│   ` / `    ` の 4 文字
        let depth = line[..connector].chars().count() / 4;
        let rest = &line[connector + "├── ".len()..];
        let name = rest.split(" #").next().unwrap_or(rest).trim();
        stack.truncate(depth);
        if name == "..." {
            continue;
        }
        let parent: String = stack.concat();
        paths.push(format!("{parent}{name}"));
        stack.push(if name.ends_with('/') {
            name.to_string()
        } else {
            format!("{name}/")
        });
    }
    paths
}

/// 1 列目の見出しに基準ディレクトリを持つ表から (基準, 項目) を取り出す
///
/// 見出しが `` | モジュール (`tray/`) | 責務 | `` のような表が対象
/// 項目は各行 1 列目のバッククォート内トークン
///
/// # Arguments
/// * `lines` - 本文の行
///
/// # Returns
/// * `Vec<(String, String)>` - (基準ディレクトリ, 基準からの相対パス)
fn parse_based_tables(lines: &[&str]) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut base: Option<String> = None;
    for line in lines {
        let Some(first_cell) = line.strip_prefix('|').and_then(|row| row.split('|').next()) else {
            base = None;
            continue;
        };
        if first_cell.trim_start().starts_with(":--") {
            continue;
        }
        match &base {
            None => {
                base = backtick_tokens(first_cell)
                    .into_iter()
                    .find(|token| token.ends_with('/'))
                    .map(str::to_string);
                // 基準を持たない表は見出し以降も対象にしない
                if base.is_none() {
                    base = Some(String::new());
                }
            }
            Some(dir) if dir.is_empty() => {}
            Some(dir) => entries.extend(
                backtick_tokens(first_cell)
                    .into_iter()
                    .map(|token| (dir.clone(), token.to_string())),
            ),
        }
    }
    entries
}

/// 基準ディレクトリ配下に項目が実在するか
///
/// 項目はそのままのパスのほか、拡張子を省いたモジュール名 (`model` → `model.rs`) も受け付ける
///
/// # Arguments
/// * `base` - 基準ディレクトリ (末尾が一致するディレクトリすべてを候補にする)
/// * `entry` - 基準からの相対パス
/// * `paths` - リポジトリ内の全パス
///
/// # Returns
/// * `bool` - 実在する場合は `true`
fn exists_under(base: &str, entry: &str, paths: &[String]) -> bool {
    [
        format!("{base}{entry}"),
        format!("{base}{entry}.rs"),
        format!("{base}{entry}/"),
    ]
    .iter()
    .any(|candidate| exists_as_suffix(candidate, paths))
}

/// ドキュメント内で実在しないパスを列挙する
///
/// # Arguments
/// * `doc` - ドキュメント全体
/// * `paths` - リポジトリ内の全パス
///
/// # Returns
/// * `Vec<String>` - 見つからないパスと検出箇所の説明
fn find_missing_paths(doc: &str, paths: &[String]) -> Vec<String> {
    let (prose, tree) = split_code_blocks(doc);
    let mut missing = Vec::new();

    // 構成図はルート相対で完全一致させる
    for path in parse_tree(&tree) {
        if !paths.iter().any(|actual| wildcard_match(&path, actual)) {
            missing.push(format!("構成図: {path}"));
        }
    }

    // 基準付きの表は基準ディレクトリ配下にあること
    for (base, entry) in parse_based_tables(&prose) {
        if !exists_under(&base, &entry, paths) {
            missing.push(format!("表 ({base}): {entry}"));
        }
    }

    // 本文・表のパスはどこかのディレクトリの末尾に一致すること
    for token in prose.iter().flat_map(|line| backtick_tokens(line)) {
        if !is_path_token(token) || RUNTIME_PATHS.contains(&token) {
            continue;
        }
        if !expand_braces(token)
            .iter()
            .all(|pattern| exists_as_suffix(pattern, paths))
        {
            missing.push(format!("本文: {token}"));
        }
    }

    missing.sort();
    missing.dedup();
    missing
}

// ======================================================================
// テスト
// ======================================================================

/// `architecture.md` に書いたパスがすべて実在すること
#[test]
fn architecture_doc_paths_exist() {
    let root = repo_root();
    let doc = fs::read_to_string(root.join(DOC))
        .unwrap_or_else(|error| panic!("{DOC} を読めない: {error}"));
    let paths = collect_repo_paths(&root);

    let missing = find_missing_paths(&doc, &paths);

    assert!(
        missing.is_empty(),
        "{DOC} に実在しないパスがある。移動・改名に合わせて更新すること:\n  {}",
        missing.join("\n  ")
    );
}

/// 構成図・基準付きの表・本文のそれぞれで欠けたパスを検出すること
#[test]
fn find_missing_paths_reports_each_source() {
    let paths: Vec<String> = [
        "crates/",
        "crates/app/",
        "crates/app/src/",
        "crates/app/src/tray/",
        "crates/app/src/tray/menu/",
        "crates/app/src/tray/model.rs",
        "crates/app/src/ui/",
        "crates/app/src/ui/a_logic.ts",
    ]
    .map(String::from)
    .to_vec();
    let doc = "\
```
root/
├── crates/
│   ├── app/          # アプリ
│   │   ├── src/
│   │   │   ├── tray/
│   │   │   └── gone/ # 削除済み
│   │   └── ...
```

| モジュール (`tray/`) | 責務 |
| :------------------- | :--- |
| `menu/` | メニュー |
| `model` | モデル |
| `missing` | 欠落 |

| 種類 | 責務 |
| :--- | :--- |
| `untracked` | 基準なしの表は 1 列目を基準付きで検証しない |

`ui/*_logic.ts` と `tray/{menu/,model.rs}` は実在し、`ui/*.css` は欠落、`logs/` は実行時のパスのため対象外
";

    let missing = find_missing_paths(doc, &paths);

    // 構成図は親をたどった完全パス、表は基準配下、本文は末尾一致で判定する
    assert_eq!(
        missing,
        [
            "本文: ui/*.css",
            "構成図: crates/app/src/gone/",
            "表 (tray/): missing",
        ]
    );
}

/// `*` はパス区切りをまたがずに照合すること
#[test]
fn wildcard_match_stays_within_segment() {
    assert!(wildcard_match("*.rs", "lib.rs"));
    assert!(wildcard_match("tests/flow/*.rs", "tests/flow/monitor.rs"));
    assert!(wildcard_match("css/*.css", "css/base.css"));
    assert!(!wildcard_match("*.rs", "src/lib.rs"));
    assert!(!wildcard_match("*.rs", "lib.js"));
}

/// パスらしいトークンだけを検証対象にすること
#[test]
fn is_path_token_accepts_only_paths() {
    // 検証対象
    assert!(is_path_token("tray/runner.rs"));
    assert!(is_path_token("features/"));
    assert!(is_path_token("*.bundle.js"));
    assert!(is_path_token("selector/{quick,clip,retention}.rs"));

    // 識別子・CLI 引数・実行時のパスなどは対象外
    assert!(!is_path_token("AppState"));
    assert!(!is_path_token("--apply-portable-update"));
    assert!(!is_path_token("sha256:<hex>"));
    assert!(!is_path_token("settings-window.json"));
    assert!(!is_path_token("text/html;base64"));
}
