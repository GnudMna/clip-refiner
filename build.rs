fn main() {
    // Windowsの場合はexeに情報を追加
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        
        // アプリ情報
        res.set("ProductName", env!("CARGO_PKG_NAME"));
        res.set("FileDescription", env!("CARGO_PKG_NAME"));

        // バージョン情報
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));

        // 著作権
        res.set("LegalCopyright", env!("CARGO_PKG_AUTHORS"));
        
        // アイコン設定
        res.set_icon("assets/icon.ico");

        res.compile().unwrap();
    }
}
