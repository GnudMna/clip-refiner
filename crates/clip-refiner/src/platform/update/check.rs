use std::path::Path;

use super::error::UpdateError;
use super::types::{
    AvailableUpdate, UpdateCheckOutcome, UpdatePackageKind, VersionApiAsset, VersionApiResponse,
};
use super::verify::{parse_asset_digest, validate_declared_download_size};
use super::{BRIDGE_BASE_URL, RELEASE_OWNER, RELEASE_REPO, current_app_version};

use semver::Version;

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

/// version API レスポンスから更新結果を判定する
///
/// # Arguments
/// * `current_raw` - 現在バージョン文字列
/// * `current` - パース済み現在バージョン
/// * `response` - version API レスポンス
/// * `kind` - 求める配布物種別
/// * `arch` - CPU アーキテクチャ文字列 (例: `x86_64`)
///
/// # Returns
/// * `UpdateCheckOutcome` - 判定結果
pub(crate) fn evaluate_update(
    current_raw: &str,
    current: &Version,
    response: &VersionApiResponse,
    kind: UpdatePackageKind,
    arch: &str,
) -> UpdateCheckOutcome {
    let latest_raw = strip_version_prefix(&response.tag);
    let Ok(latest) = parse_semver(latest_raw) else {
        return UpdateError::LatestVersionInvalid {
            tag: response.tag.clone(),
        }
        .into();
    };

    if latest <= *current {
        return UpdateCheckOutcome::UpToDate {
            current_version: current_raw.to_string(),
            latest_version: latest_raw.to_string(),
        };
    }

    let Some(asset) = select_asset(&response.assets, kind, arch) else {
        let kind_label = UpdateError::package_kind_label(kind).to_string();
        let has_kind = response
            .assets
            .iter()
            .any(|asset| asset_matches_kind(asset, kind));
        let err = if has_kind {
            UpdateError::AssetArchMissing {
                arch: arch.to_string(),
                kind_label,
                tag: response.tag.clone(),
            }
        } else {
            UpdateError::AssetMissing {
                kind_label,
                tag: response.tag.clone(),
            }
        };
        return err.into();
    };

    let Some(raw_digest) = asset.integrity_raw() else {
        return UpdateError::DigestMissing {
            asset: asset.name.clone(),
        }
        .into();
    };
    let sha256 = match parse_asset_digest(raw_digest) {
        Ok(digest) => digest,
        Err(err) => return err.into(),
    };
    if let Err(err) = validate_declared_download_size(asset.size) {
        return err.into();
    }

    UpdateCheckOutcome::Available(AvailableUpdate {
        current_version: current_raw.to_string(),
        latest_version: latest_raw.to_string(),
        tag: response.tag.clone(),
        asset_name: asset.name.clone(),
        size: asset.size,
        sha256,
    })
}

/// 配布物種別とアーキテクチャに合うアセットを選ぶ
///
/// 種別のみ一致してアーキテクチャが一致しない場合は `None` を返す
/// (先頭候補へのフォールバックはしない)
///
/// # Arguments
/// * `assets` - リリースアセット一覧
/// * `kind` - MSI / ポータブル
/// * `arch` - CPU アーキテクチャ文字列 (例: `x86_64`)
///
/// # Returns
/// * `Option<&VersionApiAsset>` - 種別とアーキテクチャの両方が一致するアセット
pub(crate) fn select_asset<'a>(
    assets: &'a [VersionApiAsset],
    kind: UpdatePackageKind,
    arch: &str,
) -> Option<&'a VersionApiAsset> {
    assets
        .iter()
        .find(|asset| asset_matches_kind(asset, kind) && asset.name.contains(arch))
}

/// タグやバージョン文字列から先頭の `v` / `V` を除去する
///
/// # Arguments
/// * `value` - タグまたはバージョン文字列
///
/// # Returns
/// * `&str` - プレフィックス除去後の文字列
pub(crate) fn strip_version_prefix(value: &str) -> &str {
    value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value)
}

/// `SemVer` 文字列をパースする
///
/// # Arguments
/// * `value` - `v` 付きでも可
///
/// # Returns
/// * `Result<Version, semver::Error>` - パース結果
pub(crate) fn parse_semver(value: &str) -> Result<Version, semver::Error> {
    Version::parse(strip_version_prefix(value.trim()))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// アセット名が配布物種別に一致するか判定する
///
/// # Arguments
/// * `asset` - リリースアセット
/// * `kind` - MSI / ポータブル
///
/// # Returns
/// * `bool` - 種別が一致すれば `true`
fn asset_matches_kind(asset: &VersionApiAsset, kind: UpdatePackageKind) -> bool {
    match kind {
        UpdatePackageKind::Msi => Path::new(&asset.name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("msi")),
        UpdatePackageKind::PortableZip => {
            asset.name.to_ascii_lowercase().ends_with("-portable.zip")
        }
    }
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
    use super::*;

    fn asset(name: &str, size: u64) -> VersionApiAsset {
        VersionApiAsset {
            name: name.to_string(),
            size,
            digest: Some(format!("sha256:{}", "a".repeat(64))),
            sha256: None,
        }
    }

    fn asset_without_digest(name: &str, size: u64) -> VersionApiAsset {
        VersionApiAsset {
            name: name.to_string(),
            size,
            digest: None,
            sha256: None,
        }
    }

    /// タグ先頭の `v` を除去できること
    #[test]
    fn strip_version_prefix_removes_leading_v() {
        assert_eq!(strip_version_prefix("v0.13.1"), "0.13.1");
        assert_eq!(strip_version_prefix("0.13.1"), "0.13.1");
        assert_eq!(strip_version_prefix("V1.0.0"), "1.0.0");
    }

    /// 同一バージョンは最新と判定すること
    #[test]
    fn evaluate_update_marks_equal_version_up_to_date() {
        let current = Version::parse("0.13.1").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset("clip-refiner-0.13.1-x86_64.msi", 100)],
        };

        let outcome = evaluate_update(
            "0.13.1",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        assert!(matches!(outcome, UpdateCheckOutcome::UpToDate { .. }));
    }

    /// 新しい版があればアセット付きで Available になること
    #[test]
    fn evaluate_update_returns_available_when_newer() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![
                asset("clip-refiner-0.13.1-x86_64.msi", 42),
                asset(
                    "clip-refiner-0.13.1-x86_64-pc-windows-msvc-portable.zip",
                    99,
                ),
            ],
        };

        let outcome = evaluate_update(
            "0.13.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Available(update) => {
                assert_eq!(update.latest_version, "0.13.1");
                assert_eq!(update.tag, "v0.13.1");
                assert_eq!(update.asset_name, "clip-refiner-0.13.1-x86_64.msi");
                assert_eq!(update.size, 42);
                assert_eq!(update.sha256, "a".repeat(64));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// 宣言サイズ 0 のアセットは Available にしないこと
    #[test]
    fn evaluate_update_errors_when_size_is_zero() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset("clip-refiner-0.13.1-x86_64.msi", 0)],
        };

        let outcome = evaluate_update(
            "0.13.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Error { message } => {
                assert!(message.contains("サイズ") || message.contains('0'));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// 絶対上限を超える宣言サイズは Available にしないこと
    #[test]
    fn evaluate_update_errors_when_size_exceeds_absolute_max() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset(
                "clip-refiner-0.13.1-x86_64.msi",
                super::super::verify::MAX_UPDATE_DOWNLOAD_BYTES + 1,
            )],
        };

        let outcome = evaluate_update(
            "0.13.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Error { message } => {
                assert!(message.contains("上限"));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// digest が無いアセットは Available にしないこと
    #[test]
    fn evaluate_update_errors_when_digest_missing() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset_without_digest("clip-refiner-0.13.1-x86_64.msi", 42)],
        };

        let outcome = evaluate_update(
            "0.13.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Error { message } => {
                assert!(message.contains("digest"));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// `sha256` フィールドだけでも Available になること
    #[test]
    fn evaluate_update_accepts_sha256_field() {
        let current = Version::parse("0.13.0").expect("semver");
        let hex = "b".repeat(64);
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![VersionApiAsset {
                name: "clip-refiner-0.13.1-x86_64.msi".to_string(),
                size: 10,
                digest: None,
                sha256: Some(hex.clone()),
            }],
        };

        let outcome = evaluate_update(
            "0.13.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Available(update) => {
                assert_eq!(update.sha256, hex);
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// ポータブル向けに `-portable.zip` を選ぶこと
    #[test]
    fn select_asset_prefers_portable_zip() {
        let assets = [
            asset("clip-refiner-0.13.1-x86_64.msi", 1),
            asset("clip-refiner-0.13.1-x86_64-pc-windows-msvc-portable.zip", 2),
        ];
        let selected = select_asset(&assets, UpdatePackageKind::PortableZip, "x86_64")
            .expect("portable asset");
        assert!(selected.name.ends_with("-portable.zip"));
    }

    /// アーキテクチャが一致するアセットを選ぶこと
    #[test]
    fn select_asset_requires_matching_arch() {
        let assets = [
            asset("clip-refiner-0.13.1-aarch64.msi", 1),
            asset("clip-refiner-0.13.1-x86_64.msi", 2),
        ];
        let selected = select_asset(&assets, UpdatePackageKind::Msi, "x86_64").expect("msi");
        assert_eq!(selected.name, "clip-refiner-0.13.1-x86_64.msi");
    }

    /// アーキテクチャ不一致時は先頭候補へフォールバックしないこと
    #[test]
    fn select_asset_returns_none_when_arch_mismatches() {
        let assets = [asset("clip-refiner-0.13.1-aarch64.msi", 1)];
        assert!(select_asset(&assets, UpdatePackageKind::Msi, "x86_64").is_none());
    }

    /// 対象アセットが無い場合はエラーになること
    #[test]
    fn evaluate_update_errors_when_package_missing() {
        let current = Version::parse("0.12.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset("clip-refiner-0.13.1-x86_64.msi", 1)],
        };

        let outcome = evaluate_update(
            "0.12.0",
            &current,
            &response,
            UpdatePackageKind::PortableZip,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Error { message } => {
                assert!(message.contains("ポータブル ZIP"));
                assert!(!message.contains("CPU"));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// 種別はあるがアーキテクチャ不一致の場合は CPU 向けエラーになること
    #[test]
    fn evaluate_update_errors_when_arch_mismatches() {
        let current = Version::parse("0.12.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset("clip-refiner-0.13.1-aarch64.msi", 1)],
        };

        let outcome = evaluate_update(
            "0.12.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        match outcome {
            UpdateCheckOutcome::Error { message } => {
                assert!(message.contains("CPU"));
                assert!(message.contains("x86_64"));
                assert!(message.contains("MSI"));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// リモートが古い場合も最新と判定すること
    #[test]
    fn evaluate_update_marks_older_remote_up_to_date() {
        let current = Version::parse("0.14.0").expect("semver");
        let response = VersionApiResponse {
            tag: "v0.13.1".to_string(),
            assets: vec![asset("clip-refiner-0.13.1-x86_64.msi", 1)],
        };

        let outcome = evaluate_update(
            "0.14.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        assert!(matches!(outcome, UpdateCheckOutcome::UpToDate { .. }));
    }

    /// 不正なタグはエラーになること
    #[test]
    fn evaluate_update_errors_on_invalid_tag() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = VersionApiResponse {
            tag: "not-a-version".to_string(),
            assets: vec![asset("clip-refiner-0.13.1-x86_64.msi", 1)],
        };

        let outcome = evaluate_update(
            "0.13.0",
            &current,
            &response,
            UpdatePackageKind::Msi,
            "x86_64",
        );
        assert!(matches!(outcome, UpdateCheckOutcome::Error { .. }));
    }

    /// `.MSI` など大文字拡張子も選ぶこと
    #[test]
    fn select_asset_accepts_uppercase_msi_extension() {
        let assets = [asset("clip-refiner-0.13.1-x86_64.MSI", 3)];
        let selected = select_asset(&assets, UpdatePackageKind::Msi, "x86_64").expect("msi");
        assert_eq!(selected.name, "clip-refiner-0.13.1-x86_64.MSI");
    }

    /// version API JSON をデシリアライズできること
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

    /// digest 無しの旧 bridge JSON もデシリアライズできること
    #[test]
    fn version_api_response_deserializes_without_digest() {
        let json = r#"{
            "tag":"v0.13.1",
            "assets":[{"name":"clip-refiner-0.13.1-x86_64.msi","size":1}]
        }"#;
        let response: VersionApiResponse = serde_json::from_str(json).expect("version API JSON");
        assert!(response.assets[0].integrity_raw().is_none());
    }

    /// `SemVer` パースが `v` 付きを受け付けること
    #[test]
    fn parse_semver_accepts_v_prefix() {
        let version = parse_semver("v1.2.3").expect("semver");
        assert_eq!(version.to_string(), "1.2.3");
    }
}
