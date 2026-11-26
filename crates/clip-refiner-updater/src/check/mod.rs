//! GitHub API の最新リリースによる更新確認
//!
//! 取得・`SemVer` 比較・アセット選択・digest / サイズ検証を担う

mod asset;
mod evaluate;
mod version;

use super::error::UpdateError;
use super::http;
use super::types::{LatestRelease, UpdateCheckOutcome, UpdatePackageKind};
use super::{GITHUB_API_BASE_URL, RELEASE_OWNER, RELEASE_REPO, current_app_version};
use evaluate::evaluate_update;
use version::parse_semver;

// ======================================================================
// パブリック関数
// ======================================================================
/// GitHub の最新リリースで更新の有無を確認する
///
/// ドラフトとプレリリースは GitHub API 側で対象外になる
///
/// # Arguments
/// * `kind` - 求める配布物種別
///
/// # Returns
/// * `UpdateCheckOutcome` - 最新 / 更新あり / エラー
pub(crate) fn check_for_update(kind: UpdatePackageKind) -> UpdateCheckOutcome {
    check_for_update_at(
        GITHUB_API_BASE_URL,
        current_app_version(),
        kind,
        std::env::consts::ARCH,
    )
}

/// 指定した API ベース URL で更新の有無を確認する
///
/// 統合テストからローカル HTTP サーバへ向けて呼び出す
///
/// # Arguments
/// * `base_url` - GitHub API のベース URL (末尾スラッシュなし)
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

    let url = latest_release_url(base_url);
    let response = match fetch_latest_release(&url) {
        Ok(response) => response,
        Err(err) => return err.into(),
    };

    evaluate_update(current_raw, &current, &response, kind, arch)
}

/// 最新リリースの API URL を組み立てる
///
/// # Arguments
/// * `base_url` - GitHub API のベース URL (末尾スラッシュなし)
///
/// # Returns
/// * `String` - 最新リリースの API URL
pub(crate) fn latest_release_url(base_url: &str) -> String {
    format!("{base_url}/repos/{RELEASE_OWNER}/{RELEASE_REPO}/releases/latest")
}

/// 最新リリースを取得してデシリアライズする
///
/// `X-GitHub-Api-Version` は送らない。版のサポート終了後は `410` になり、
/// 配布済みのアプリが更新できなくなるため
fn fetch_latest_release(url: &str) -> Result<LatestRelease, UpdateError> {
    let request = http::api_agent(url)
        .get(url)
        .header("Accept", "application/vnd.github+json");
    let mut response = request.call().map_err(|err| {
        tracing::warn!("更新確認 HTTP 失敗: {err}");
        match err {
            // 認証なしの GitHub API は接続元 IP ごとの回数制限の超過を 403 / 429 で返す
            ureq::Error::StatusCode(status @ (403 | 429)) => {
                UpdateError::CheckRateLimited { status }
            }
            ureq::Error::StatusCode(status) => UpdateError::CheckHttp { status },
            _ => UpdateError::CheckNetwork,
        }
    })?;

    response
        .body_mut()
        .read_json::<LatestRelease>()
        .map_err(|err| {
            tracing::warn!("更新確認 JSON 解釈失敗: {err}");
            UpdateError::InvalidJson
        })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::GITHUB_API_BASE_URL;
    use super::super::types::LatestRelease;
    use super::latest_release_url;

    /// 最新リリースの URL が GitHub API のパスに従うようにすること
    #[test]
    fn latest_release_url_matches_github_api() {
        assert_eq!(
            latest_release_url(GITHUB_API_BASE_URL),
            "https://api.github.com/repos/GnudMna/clip-refiner/releases/latest"
        );
    }

    /// GitHub API の JSON をデシリアライズできるようにすること
    #[test]
    fn latest_release_deserializes_github_json() {
        let json = r#"{
            "url":"https://api.github.com/repos/GnudMna/clip-refiner/releases/1",
            "tag_name":"v0.13.1",
            "name":"v0.13.1",
            "draft":false,
            "prerelease":false,
            "published_at":"2026-07-25T20:36:42Z",
            "assets":[{"name":"clip-refiner-0.13.1-x86_64.msi","label":null,"size":3776512,"content_type":"application/octet-stream","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]
        }"#;
        let release: LatestRelease = serde_json::from_str(json).expect("release JSON");
        assert_eq!(release.tag_name, "v0.13.1");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "clip-refiner-0.13.1-x86_64.msi");
        assert_eq!(release.assets[0].size, 3_776_512);
        assert_eq!(
            release.assets[0].digest.as_deref(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    /// digest が無い・`null` のアセットもデシリアライズできるようにすること
    #[test]
    fn latest_release_deserializes_without_digest() {
        let json = r#"{
            "tag_name":"v0.13.1",
            "assets":[
                {"name":"clip-refiner-0.13.1-x86_64.msi","size":1},
                {"name":"clip-refiner-0.13.1-x86_64.msi.minisig","size":1,"digest":null}
            ]
        }"#;
        let release: LatestRelease = serde_json::from_str(json).expect("release JSON");
        assert!(release.assets.iter().all(|asset| asset.digest.is_none()));
    }
}
