fn main() {
    // Windowsの場合はexeに情報を追加
    #[cfg(windows)]
    {
        const APP_NAME: &str = "ClipRefiner";

        let mut res = winres::WindowsResource::new();

        // アプリ情報
        res.set("ProductName", APP_NAME);
        res.set("FileDescription", APP_NAME);

        // バージョン情報
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));

        // 著作権
        res.set("LegalCopyright", env!("CARGO_PKG_AUTHORS"));

        // アイコン設定
        res.set_icon("assets/icon.ico");

        res.compile()
            .expect("Windowsリソースのコンパイルに失敗しました");
    }
}
