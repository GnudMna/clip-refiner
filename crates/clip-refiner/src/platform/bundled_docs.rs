//! 配布物に同梱する文書 (利用者向けドキュメント・`LICENSE`・`THIRD_PARTY_NOTICES`) の起動
//!
//! ポータブル ZIP は実行ファイルと同じフォルダ、MSI は `bin\` の親フォルダに文書を置く

use std::io;
use std::path::{Path, PathBuf};

use super::shell::{shell_open, shell_open_with};

// ======================================================================
// 型
// ======================================================================
/// 配布物に同梱する文書
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledDocument {
    /// 利用者向けドキュメント (`docs\index.html`)
    UserDocs,
    /// 本体のライセンス (`LICENSE`)
    License,
    /// 第三者ライセンス表示 (`THIRD_PARTY_NOTICES`)
    ThirdPartyNotices,
}

impl BundledDocument {
    /// 配布物のルートフォルダからの相対パス
    fn relative_path(self) -> &'static str {
        match self {
            Self::UserDocs => r"docs\index.html",
            Self::License => "LICENSE",
            Self::ThirdPartyNotices => "THIRD_PARTY_NOTICES",
        }
    }
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 同梱文書を既定のアプリケーションで開く
///
/// # Arguments
/// * `document` - 開く文書
///
/// # Returns
/// * `io::Result<()>` - 起動を受け付けた場合は `Ok(())`。実行ファイルのフォルダとその親に文書が無い場合 (開発ビルドなど) は `NotFound`
pub(crate) fn open_bundled_document(document: BundledDocument) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "実行ファイルのフォルダがない"))?;
    let path = existing_document_path(exe_dir, document)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "パスが UTF-8 でない"))?;
    match document {
        BundledDocument::UserDocs => shell_open(path_str),
        // 拡張子が無く関連付けも無いため、メモ帳で明示的に開く
        BundledDocument::License | BundledDocument::ThirdPartyNotices => {
            shell_open_with(&notepad_path()?, path_str)
        }
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// メモ帳のフルパスを返す
///
/// 検索パス上の偽物を拾わないよう、`SystemRoot` 配下の実体を指定する
fn notepad_path() -> io::Result<String> {
    let root = std::env::var_os("SystemRoot")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SystemRoot が未設定"))?;
    Path::new(&root)
        .join("notepad.exe")
        .into_os_string()
        .into_string()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "パスが UTF-8 でない"))
}

/// 実行ファイルのフォルダ、次にその親フォルダから同梱文書のパスを探す
///
/// ポータブル ZIP は実行ファイルと同じフォルダ、MSI は `bin\` の親フォルダに文書がある
fn existing_document_path(exe_dir: &Path, document: BundledDocument) -> io::Result<PathBuf> {
    let relative = document.relative_path();
    exe_dir
        .ancestors()
        .take(2)
        .map(|dir| dir.join(relative))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "同梱文書が見つからない: {}",
                    exe_dir.join(relative).display()
                ),
            )
        })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// 文書が実行ファイルのフォルダか、その親フォルダにあるときはそのパスを返し、無いときは `NotFound` にすること
    #[test]
    fn existing_document_path_requires_file_next_to_exe() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-bundled-docs-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("docs")).expect("temp dir");

        // 何も置いていない状態
        for document in [
            BundledDocument::UserDocs,
            BundledDocument::License,
            BundledDocument::ThirdPartyNotices,
        ] {
            let err = existing_document_path(&root, document).expect_err("missing");
            assert_eq!(err.kind(), io::ErrorKind::NotFound);
        }

        // 配布物と同じ配置にした状態
        fs::write(root.join("docs").join("index.html"), b"<html>").expect("write index");
        fs::write(root.join("LICENSE"), b"license").expect("write license");
        fs::write(root.join("THIRD_PARTY_NOTICES"), b"notices").expect("write notices");
        assert_eq!(
            existing_document_path(&root, BundledDocument::UserDocs).expect("docs"),
            root.join("docs").join("index.html")
        );
        assert_eq!(
            existing_document_path(&root, BundledDocument::License).expect("license"),
            root.join("LICENSE")
        );
        assert_eq!(
            existing_document_path(&root, BundledDocument::ThirdPartyNotices).expect("notices"),
            root.join("THIRD_PARTY_NOTICES")
        );

        // MSI と同じ配置 (実行ファイルが bin\ 配下、文書は親フォルダ)
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin dir");
        assert_eq!(
            existing_document_path(&bin, BundledDocument::UserDocs).expect("msi docs"),
            root.join("docs").join("index.html")
        );
        assert_eq!(
            existing_document_path(&bin, BundledDocument::License).expect("msi license"),
            root.join("LICENSE")
        );
        assert_eq!(
            existing_document_path(&bin, BundledDocument::ThirdPartyNotices).expect("msi notices"),
            root.join("THIRD_PARTY_NOTICES")
        );

        let _ = fs::remove_dir_all(&root);
    }
}
