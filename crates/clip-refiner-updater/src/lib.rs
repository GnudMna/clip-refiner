//! GitHub Releases によるアプリ更新の確認・適用
//!
//! 常駐アプリは `launch_updater` で更新プログラム (`ClipRefinerUpdater.exe`) を起動するだけで、
//! 確認からインストール・再起動・結果の表示までは更新プログラム (`app`) が担う
//!
//! GitHub API が返す各アセットに `digest` (`sha256:<hex>`) が含まれることを前提とする
//! ダウンロード中は宣言サイズ超過または絶対上限で打ち切り、
//! 完了後にサイズと SHA-256 を検証してから適用する
//! 適用前に `<asset>.minisig` の minisign 署名を埋め込み公開鍵で検証し、
//! 信頼コメントで更新先バージョンとアセット名が一致することを確かめる

#[cfg(not(target_os = "windows"))]
compile_error!("ClipRefiner は Windows 専用です");

mod app;
mod check;
mod error;
mod http;
mod installed;
mod msi;
mod named_event;
mod prepare;
mod replace;
mod resident;
mod session;
mod signature;
mod types;
mod verify;

#[cfg(any(test, feature = "test-helpers"))]
#[doc(hidden)]
pub mod test_helpers;

pub use app::run_updater;
pub use error::UpdateError;
pub use replace::run_portable_replace;
pub use session::{UpdaterSession, UpdaterSessionEnd, launch_updater, notify_startup_complete};
pub use types::{AvailableUpdate, PreparedPackage, UpdateCheckOutcome, UpdatePackageKind};

use std::path::PathBuf;

use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

// ======================================================================
// 定数
// ======================================================================
/// GitHub REST API のベース URL (最新リリースの照会)
pub(crate) const GITHUB_API_BASE_URL: &str = "https://api.github.com";
/// GitHub のベース URL (リリースアセットのダウンロード)
pub(crate) const GITHUB_BASE_URL: &str = "https://github.com";
/// GitHub リポジトリ所有者
pub(crate) const RELEASE_OWNER: &str = "GnudMna";
/// GitHub リポジトリ名
pub(crate) const RELEASE_REPO: &str = "clip-refiner";
/// 更新作業ディレクトリ名 (`%TEMP%` 直下)
pub const UPDATE_WORK_DIR_NAME: &str = "clip-refiner-update";

/// ビルド時に埋め込まれたアプリバージョンを返す
///
/// # Returns
/// * `&'static str` - Cargo パッケージバージョン
pub(crate) fn current_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// システムフォルダ内の実行ファイルの絶対パスを返す
///
/// 実行ファイルと同じフォルダの同名ファイルや `PATH` 上の偽物を拾わないよう、
/// 外部コマンドはこのパスで起動する
///
/// # Arguments
/// * `file_name` - 実行ファイル名 (例: `msiexec.exe`)
///
/// # Returns
/// * `Option<PathBuf>` - システムフォルダを取得できない場合は `None`
pub(crate) fn system_exe_path(file_name: &str) -> Option<PathBuf> {
    let mut buf = [0u16; 260];
    // SAFETY: `buf` は呼び出し中有効な書き込み可能バッファで、長さはスライスから渡す
    let len = usize::try_from(unsafe { GetSystemDirectoryW(Some(&mut buf)) }).ok()?;
    if len == 0 || len >= buf.len() {
        return None;
    }
    let dir = PathBuf::from(String::from_utf16(&buf[..len]).ok()?);
    Some(dir.join(file_name))
}
