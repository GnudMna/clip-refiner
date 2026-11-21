#[cfg(not(target_os = "windows"))]
compile_error!("ClipRefiner は Windows 専用です");

/// Windows リソースの著作権表記。`LICENSE` と揃えて更新する
const LEGAL_COPYRIGHT: &str = "Copyright (c) 2026 Yuto Hirata";

// PascalCase の場合は定数として直接記述する
const APP_NAME: &str = "ClipRefiner";

/// アイコンパス
const ICON_PATH: &str = "../../assets/icon.ico";

fn main() {
    // アイコンパスを再実行条件に追加
    println!("cargo:rerun-if-changed={ICON_PATH}");

    let mut res = winres::WindowsResource::new();

    // アイコン
    res.set_icon(ICON_PATH);

    // アプリ情報
    res.set("ProductName", APP_NAME);
    res.set("FileDescription", APP_NAME);

    // バージョン情報
    res.set("FileVersion", env!("CARGO_PKG_VERSION"));
    res.set("ProductVersion", env!("CARGO_PKG_VERSION"));

    // 著作権(メールアドレス形式の authors ではなく, LICENSE と同じ表記を使う)
    res.set("LegalCopyright", LEGAL_COPYRIGHT);

    if let Err(error) = res.compile() {
        panic!("Windows リソースの埋め込みに失敗: {error}");
    }
}
