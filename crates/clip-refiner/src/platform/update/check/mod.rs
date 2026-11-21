//! gh-release-bridge version API による更新確認
//!
//! 取得・`SemVer` 比較・アセット選択・digest / サイズ検証を担う

mod asset;
mod evaluate;
mod version;

use super::error::UpdateError;
use super::types::{UpdateCheckOutcome, UpdatePackageKind, VersionApiResponse};
use super::{BRIDGE_BASE_URL, RELEASE_OWNER, RELEASE_REPO, current_app_version};
use evaluate::evaluate_update;
use version::parse_semver;

// ======================================================================
// パブリック関数
// ======================================================================
/// gh-release-bridge の version API で更新の有無を確認する
///
/// # Returns
/// * `UpdateCheckOutcome` - 最新 / 更新あり / エラー
pub(crate) fn check_for_update() -> UpdateCheckOutcome {
    check_for_update_at(
        BRIDGE_BASE_URL,
        current_app_version(),
        UpdatePackageKind::current(),
        std::env::consts::ARCH,
    )
}

/// 指定した bridge ベース URL で更新の有無を確認する
///
/// 統合テストからローカル HTTP サーバへ向けて呼び出す
///
/// # Arguments
/// * `base_url` - bridge のベース URL (末尾スラッシュなし)
/// * `current_raw` - 現在のアプリバージョン文字列
/// * `kind` - 求める配布物種別
/// * `arch` - CPU アーキテクチャ文字列 (例: `x86_64`)
///
/// # Returns
/// * `UpdateCheckOutcome` - 最新 / 更新あり / エラー
pub(crate) fn check_for_update_at(
    base_url: &str,
    current_raw: &str,
    kind: UpdatePackageKind,
    arch: &str,
) -> UpdateCheckOutcome {
    let Ok(current) = parse_semver(current_raw) else {
        return UpdateError::CurrentVersionInvalid {
            version: current_raw.to_string(),
        }
        .into();
    };

    let url = version_api_url(base_url);
    let response = match fetch_version_api(&url) {
        Ok(response) => response,
        Err(err) => return err.into(),
    };

    evaluate_update(current_raw, &current, &response, kind, arch)
}

/// version API の URL を組み立てる
///
/// # Arguments
/// * `base_url` - bridge のベース URL (末尾スラッシュなし)
///
/// # Returns
/// * `String` - version API URL
pub(crate) fn version_api_url(base_url: &str) -> String {
    format!("{base_url}/{RELEASE_OWNER}/{RELEASE_REPO}/version")
}

/// version API を取得してデシリアライズする
fn fetch_version_api(url: &str) -> Result<VersionApiResponse, UpdateError> {
    let mut response = ureq::get(url).call().map_err(|err| {
        crate::log_warn!("更新確認 HTTP 失敗: {err}");
        match err {
            ureq::Error::StatusCode(status) => UpdateError::CheckHttp { status },
            _ => UpdateError::CheckNetwork,
        }
    })?;

    response
        .body_mut()
        .read_json::<VersionApiResponse>()
        .map_err(|err| {
            crate::log_warn!("更新確認 JSON 解釈失敗: {err}");
            UpdateError::InvalidJson
        })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::types::VersionApiResponse;

    /// version API JSON をデシリアライズできるようにすること
    #[test]
    fn version_api_response_deserializes_bridge_json() {
        let json = r#"{
            "owner":"GnudMna",
            "repo":"clip-refiner",
            "tag":"v0.13.1",
            "name":"v0.13.1",
            "prerelease":false,
            "published_at":"2026-07-25T20:36:42Z",
            "assets":[{"name":"clip-refiner-0.13.1-x86_64.msi","size":3776512,"content_type":"application/octet-stream","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]
        }"#;
        let response: VersionApiResponse = serde_json::from_str(json).expect("version API JSON");
        assert_eq!(response.tag, "v0.13.1");
        assert_eq!(response.assets.len(), 1);
        assert_eq!(response.assets[0].name, "clip-refiner-0.13.1-x86_64.msi");
        assert_eq!(response.assets[0].size, 3_776_512);
        assert_eq!(
            response.assets[0].integrity_raw(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    /// digest 無しの旧 bridge JSON もデシリアライズできるようにすること
    #[test]
    fn version_api_response_deserializes_without_digest() {
        let json = r#"{
            "tag":"v0.13.1",
            "assets":[{"name":"clip-refiner-0.13.1-x86_64.msi","size":1}]
        }"#;
        let response: VersionApiResponse = serde_json::from_str(json).expect("version API JSON");
        assert!(response.assets[0].integrity_raw().is_none());
    }
}
