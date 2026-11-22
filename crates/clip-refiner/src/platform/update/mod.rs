//! gh-release-bridge 経由のアプリ更新確認・適用
//!
//! version API の各アセットに GitHub Release Asset の `digest` (`sha256:<hex>`) が
//! 含まれることを前提とする。ダウンロード中は宣言サイズ超過または絶対上限で打ち切り、
//! 完了後にサイズと SHA-256 を検証してから適用する
//! 保護対象タグ向けにビルド時のキー (`CLIP_REFINER_KEY_FILE` が指すファイル) を ChaCha20-Poly1305 で暗号化して埋め込み、
//! 埋め込まれていればアセット・署名の DL で復号した値を `Authorization: Bearer` で付与する
//! 適用前に `<asset>.minisig` の minisign 署名を埋め込み公開鍵で検証し、
//! 信頼コメントで更新先バージョンとアセット名が一致することを確かめる

mod apply;
mod bridge_key;
mod bridge_key_obfuscate;
mod check;
mod error;
mod http;
mod replace;
mod signature;
mod types;
mod verify;

pub(crate) use apply::apply_update;
pub(crate) use bridge_key::bridge_download_key;
pub(crate) use check::check_for_update;
pub(crate) use error::UpdateError;
pub(crate) use replace::run_portable_replace;
pub use types::{AvailableUpdate, UpdateApplyOutcome, UpdateCheckOutcome};
pub(crate) use types::{UpdateApplyUiResponse, UpdateCheckUiResponse};

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use apply::ApplyOptions;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use apply::apply_update_at;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use apply::apply_update_at_with_key;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use check::check_for_update_at;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use signature::{expected_trusted_comment, signature_asset_name};
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use types::UpdatePackageKind;

use std::path::PathBuf;

use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

// ======================================================================
// 定数
// ======================================================================
/// gh-release-bridge のベース URL
pub(crate) const BRIDGE_BASE_URL: &str = "https://gh-release-bridge.gnudlemna-4656.workers.dev";
/// GitHub リポジトリ所有者
pub(crate) const RELEASE_OWNER: &str = "GnudMna";
/// GitHub リポジトリ名
pub(crate) const RELEASE_REPO: &str = "clip-refiner";
/// 更新作業ディレクトリ名 (`%TEMP%` 直下)
pub(crate) const UPDATE_WORK_DIR_NAME: &str = "clip-refiner-update";

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
