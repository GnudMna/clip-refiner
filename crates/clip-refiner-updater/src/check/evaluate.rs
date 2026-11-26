//! 最新リリースの情報から更新の有無を判定する

use semver::Version;

use super::super::error::UpdateError;
use super::super::types::{AvailableUpdate, LatestRelease, UpdateCheckOutcome, UpdatePackageKind};
use super::super::verify::{
    parse_asset_digest, validate_asset_name, validate_declared_download_size,
};
use super::asset::{asset_matches_kind, select_asset};

/// 最新リリースの情報から更新結果を判定する
///
/// # Arguments
/// * `current_raw` - 現在バージョン文字列
/// * `current` - パース済み現在バージョン
/// * `response` - 最新リリースの情報
/// * `kind` - 求める配布物種別
/// * `arch` - CPU アーキテクチャ文字列 (例: `x86_64`)
///
/// # Returns
/// * `UpdateCheckOutcome` - 判定結果
pub(crate) fn evaluate_update(
    current_raw: &str,
    current: &Version,
    response: &LatestRelease,
    kind: UpdatePackageKind,
    arch: &str,
) -> UpdateCheckOutcome {
    // タグは URL と作業ディレクトリ名にそのまま使うため、`v` + `SemVer` の厳密な形だけを受け付ける
    // (空白付き・`V` 付き・`v` なしを正規化して通さない)
    let Some((latest_raw, latest)) = response
        .tag_name
        .strip_prefix('v')
        .and_then(|raw| Version::parse(raw).ok().map(|version| (raw, version)))
    else {
        return UpdateError::LatestVersionInvalid {
            tag: response.tag_name.clone(),
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
                tag: response.tag_name.clone(),
            }
        } else {
            UpdateError::AssetMissing {
                kind_label,
                tag: response.tag_name.clone(),
            }
        };
        return err.into();
    };

    if let Err(err) = validate_asset_name(&asset.name) {
        return err.into();
    }
    let Some(raw_digest) = asset.digest.as_deref() else {
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
        tag: response.tag_name.clone(),
        asset_name: asset.name.clone(),
        size: asset.size,
        sha256,
    })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use semver::Version;

    use super::super::super::types::{
        LatestRelease, ReleaseAsset, UpdateCheckOutcome, UpdatePackageKind,
    };
    use super::super::super::verify::MAX_UPDATE_DOWNLOAD_BYTES;
    use super::evaluate_update;

    fn asset(name: &str, size: u64) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            size,
            digest: Some(format!("sha256:{}", "a".repeat(64))),
        }
    }

    fn asset_without_digest(name: &str, size: u64) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            size,
            digest: None,
        }
    }

    /// 同一バージョンは最新と判定すること
    #[test]
    fn evaluate_update_marks_equal_version_up_to_date() {
        let current = Version::parse("0.13.1").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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

    /// 新しい版があればアセット付きで Available になるようにすること
    #[test]
    fn evaluate_update_returns_available_when_newer() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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

    /// 宣言サイズ 0 のアセットは Available にしないようにすること
    #[test]
    fn evaluate_update_errors_when_size_is_zero() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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

    /// 絶対上限を超える宣言サイズは Available にしないようにすること
    #[test]
    fn evaluate_update_errors_when_size_exceeds_absolute_max() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
            assets: vec![asset(
                "clip-refiner-0.13.1-x86_64.msi",
                MAX_UPDATE_DOWNLOAD_BYTES + 1,
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

    /// digest が無いアセットは Available にしないようにすること
    #[test]
    fn evaluate_update_errors_when_digest_missing() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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

    /// パス区切りを含むアセット名は Available にしないようにすること
    #[test]
    fn evaluate_update_errors_when_asset_name_is_path() {
        let current = Version::parse("0.13.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
            assets: vec![asset("..\\..\\clip-refiner-0.13.1-x86_64.msi", 42)],
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
                assert!(message.contains("アセット名"));
            }
            other => panic!("想定外: {other:?}"),
        }
    }

    /// 対象アセットが無い場合はエラーになるようにすること
    #[test]
    fn evaluate_update_errors_when_package_missing() {
        let current = Version::parse("0.12.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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

    /// 種別はあるがアーキテクチャ不一致の場合は CPU 向けエラーになるようにすること
    #[test]
    fn evaluate_update_errors_when_arch_mismatches() {
        let current = Version::parse("0.12.0").expect("semver");
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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
        let response = LatestRelease {
            tag_name: "v0.13.1".to_string(),
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

    /// `v` + `SemVer` の厳密な形でないタグはエラーになるようにすること
    ///
    /// 空白付き・`V` 付き・`v` なし・パス区切り入りのタグは正規化して通さない
    #[test]
    fn evaluate_update_errors_on_invalid_tag() {
        let current = Version::parse("0.13.0").expect("semver");

        for tag in [
            "not-a-version",
            " v0.13.1",
            "v0.13.1 ",
            "V0.13.1",
            "0.13.1",
            "vv0.13.1",
            "v0.13.1/../x",
        ] {
            let response = LatestRelease {
                tag_name: tag.to_string(),
                assets: vec![asset("clip-refiner-0.13.1-x86_64.msi", 1)],
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
                    assert!(message.contains("最新バージョンを解釈できません"), "{tag}");
                }
                other => panic!("想定外 ({tag}): {other:?}"),
            }
        }
    }
}
