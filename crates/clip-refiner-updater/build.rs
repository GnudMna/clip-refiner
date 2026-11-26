#![allow(
    clippy::expect_used,
    reason = "ビルドスクリプトは失敗時に panic してビルドを中断するのが正規の挙動"
)]

use std::env;
use std::fmt::Write;
use std::fs;
use std::path::Path;

// ======================================================================
// 定数
// ======================================================================
/// 更新署名の公開鍵 (minisign 形式 `*.pub`) を置くディレクトリ
const UPDATE_SIGNING_KEYS_DIR: &str = "../../keys/update-signing";

/// minisign 公開鍵の Base64 長 (署名方式 2 + 鍵 ID 8 + Ed25519 公開鍵 32 バイト)
const MINISIGN_PUBLIC_KEY_B64_LEN: usize = 56;

/// Windows リソースの著作権表記。`LICENSE` と揃えて更新する
const LEGAL_COPYRIGHT: &str = "Copyright (c) 2026 Yuto Hirata";

/// 更新プログラムのバイナリ名
const UPDATER_BIN_NAME: &str = "ClipRefinerUpdater";

/// アイコンパス
const ICON_PATH: &str = "../../assets/icon.ico";

/// 更新プログラムのマニフェスト
///
/// 更新画面の `TaskDialogIndirect` は Common Controls v6 にしか無い
/// `asInvoker` は、名前に `Updater` を含む実行ファイルへの昇格の要求を防ぐ
const MANIFEST_PATH: &str = "ClipRefinerUpdater.manifest";

fn main() {
    println!("cargo:rerun-if-changed={UPDATE_SIGNING_KEYS_DIR}");
    println!("cargo:rerun-if-changed={ICON_PATH}");
    println!("cargo:rerun-if-changed={MANIFEST_PATH}");

    warn_if_crt_not_static();
    embed_update_signing_keys();
    embed_windows_resources();
}

// ======================================================================
// CRT の静的リンク
// ======================================================================
/// MSVC ターゲットで CRT が静的リンクされない場合に警告する
///
/// 環境変数 `RUSTFLAGS` などがあると `.cargo/config.toml` の rustflags は使われず、
/// 気付かないまま VC++ 再頒布可能パッケージに依存する exe になるため
fn warn_if_crt_not_static() {
    let crt_static = env::var("CARGO_CFG_TARGET_FEATURE")
        .is_ok_and(|features| features.split(',').any(|feature| feature == "crt-static"));
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") && !crt_static {
        println!(
            "cargo:warning=CRT が静的リンクされません。環境変数 RUSTFLAGS などがあると .cargo/config.toml の rustflags は使われないため、必要なら -C target-feature=+crt-static を加えてください"
        );
    }
}

// ======================================================================
// Windows リソース
// ======================================================================
/// アイコン・バージョン情報・マニフェストを更新プログラムへ埋め込む
///
/// ライブラリとして使う `ClipRefiner.exe` のリソースと衝突しないよう、対象をバイナリに限る
fn embed_windows_resources() {
    let mut res = winresource::WindowsResource::new();

    res.set_icon(ICON_PATH);
    res.set_manifest_file(MANIFEST_PATH);
    res.set("ProductName", "ClipRefiner");
    res.set("FileDescription", "ClipRefiner の更新");
    res.set("FileVersion", env!("CARGO_PKG_VERSION"));
    res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
    res.set("LegalCopyright", LEGAL_COPYRIGHT);

    if let Err(error) = res.compile_for(UPDATER_BIN_NAME) {
        panic!("Windows リソースの埋め込みに失敗: {error}");
    }
}

// ======================================================================
// 更新署名の公開鍵
// ======================================================================
/// `keys/update-signing/*.pub` の公開鍵を `OUT_DIR` へ書き出す
///
/// ファイル名順に並べる。ディレクトリや `*.pub` が無ければ空の一覧になる (署名検証は常に失敗)
fn embed_update_signing_keys() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("update_signing_keys.rs");

    let mut paths = match fs::read_dir(UPDATE_SIGNING_KEYS_DIR) {
        Ok(entries) => entries
            .map(|entry| entry.expect("更新署名鍵ディレクトリの列挙").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "pub"))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    paths.sort();

    let mut source = String::from(
        "// build.rs が生成する。keys/update-signing/*.pub の minisign 公開鍵 (Base64)\n\
         static UPDATE_SIGNING_KEYS: &[&str] = &[\n",
    );
    for path in &paths {
        let key = read_minisign_public_key(path);
        writeln!(source, "    \"{key}\",").expect("write key");
    }
    source.push_str("];\n");

    fs::write(&dest, source).unwrap_or_else(|err| {
        panic!("更新署名鍵埋め込みファイルの書き込みに失敗: {err}");
    });
}

/// minisign 形式の公開鍵ファイルから Base64 行を取り出す
///
/// 1 行目は `untrusted comment:`、2 行目が鍵本体。形式が不正ならビルドを止める
///
/// # Arguments
/// * `path` - `*.pub` ファイル
///
/// # Returns
/// * `String` - 公開鍵の Base64 文字列
fn read_minisign_public_key(path: &Path) -> String {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("更新署名鍵の読み込みに失敗: {}: {err}", path.display()));
    let mut lines = text.lines();
    let comment = lines.next().unwrap_or_default();
    let key = lines.next().unwrap_or_default().trim();
    let valid = comment.starts_with("untrusted comment:")
        && key.len() == MINISIGN_PUBLIC_KEY_B64_LEN
        && key.starts_with("RW")
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=');
    assert!(
        valid,
        "エラー: minisign 公開鍵の形式が不正です: {}",
        path.display()
    );
    key.to_string()
}
