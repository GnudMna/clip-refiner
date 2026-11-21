//! gh-release-bridge 経由のアプリ更新確認・適用
//!
//! version API の各アセットに GitHub Release Asset の `digest` (`sha256:<hex>`) が
//! 含まれることを前提とする。ダウンロード中は宣言サイズ超過または絶対上限で打ち切り、
//! 完了後にサイズと SHA-256 を検証してから適用する
//! 保護対象タグのアセット DL にはビルド時埋め込みキー (`CLIP_REFINER_KEY`) を
//! ChaCha20-Poly1305 で暗号化して埋め込み、復号した値を `Authorization: Bearer` で付与する
//! 適用前に `<asset>.minisig` の minisign 署名を埋め込み公開鍵で検証し、
//! 信頼コメントで更新先バージョンとアセット名が一致することを確かめる

mod apply;
mod bridge_key;
mod bridge_key_obfuscate;
mod check;
mod error;
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
pub(crate) use types::{UpdateApplyUiResponse, UpdateCheckSource, UpdateCheckUiResponse};

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
