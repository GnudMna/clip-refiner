use std::env;

#[cfg(not(target_os = "windows"))]
compile_error!("ClipRefiner は Windows 専用です");

// ======================================================================
// 定数
// ======================================================================
/// Windows リソースの著作権表記。`LICENSE` と揃えて更新する
const LEGAL_COPYRIGHT: &str = "Copyright (c) 2026 Yuto Hirata";

// PascalCase の場合は定数として直接記述する
const APP_NAME: &str = "ClipRefiner";

/// アイコンパス
const ICON_PATH: &str = "../../assets/icon.ico";

fn main() {
    println!("cargo:rerun-if-changed={ICON_PATH}");
    println!("cargo:rerun-if-changed=../../assets/icon.png");

    warn_if_crt_not_static();
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
/// アイコンとバージョン情報を実行ファイルへ埋め込む
fn embed_windows_resources() {
    let mut res = winresource::WindowsResource::new();

    res.set_icon(ICON_PATH);
    res.set("ProductName", APP_NAME);
    res.set("FileDescription", APP_NAME);
    res.set("FileVersion", env!("CARGO_PKG_VERSION"));
    res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
    res.set("LegalCopyright", LEGAL_COPYRIGHT);

    if let Err(error) = res.compile() {
        panic!("Windows リソースの埋め込みに失敗: {error}");
    }
}
