//! 配布物に同梱する文書 (利用者向けドキュメント・`THIRD_PARTY_NOTICES`) の起動
//!
//! MSI とポータブル ZIP のどちらも、実行ファイルと同じフォルダに文書を置く

use std::io;
use std::path::{Path, PathBuf};

use super::shell::shell_open;

// ======================================================================
// 型
// ======================================================================
/// 配布物に同梱する文書
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledDocument {
    /// 利用者向けドキュメント (`docs\index.html`)
    UserDocs,
    /// 第三者ライセンス表示 (`THIRD_PARTY_NOTICES`)
    ThirdPartyNotices,
}

impl BundledDocument {
    /// 実行ファイルのフォルダからの相対パス
    fn relative_path(self) -> &'static str {
        match self {
            Self::UserDocs => r"docs\index.html",
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
/// * `io::Result<()>` - 起動を受け付けた場合は `Ok(())`。実行ファイルのフォルダに文書が無い場合 (開発ビルドなど) は `NotFound`
pub(crate) fn open_bundled_document(document: BundledDocument) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "実行ファイルのフォルダがない"))?;
    let path = existing_document_path(exe_dir, document)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "パスが UTF-8 でない"))?;
    shell_open(path_str)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 実行ファイルのフォルダにある同梱文書のパスを返す
fn existing_document_path(exe_dir: &Path, document: BundledDocument) -> io::Result<PathBuf> {
    let path = exe_dir.join(document.relative_path());
    if path.is_file() {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("同梱文書が見つからない: {}", path.display()),
        ))
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// 文書が実行ファイルのフォルダにあるときはそのパスを返し、無いときは `NotFound` にすること
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
            BundledDocument::ThirdPartyNotices,
        ] {
            let err = existing_document_path(&root, document).expect_err("missing");
            assert_eq!(err.kind(), io::ErrorKind::NotFound);
        }

        // 配布物と同じ配置にした状態
        fs::write(root.join("docs").join("index.html"), b"<html>").expect("write index");
        fs::write(root.join("THIRD_PARTY_NOTICES"), b"notices").expect("write notices");
        assert_eq!(
            existing_document_path(&root, BundledDocument::UserDocs).expect("docs"),
            root.join("docs").join("index.html")
        );
        assert_eq!(
            existing_document_path(&root, BundledDocument::ThirdPartyNotices).expect("notices"),
            root.join("THIRD_PARTY_NOTICES")
        );

        let _ = fs::remove_dir_all(&root);
    }
}
