//! リリースアセットの種別・アーキテクチャ選択

use std::path::Path;

use super::super::types::{UpdatePackageKind, VersionApiAsset};

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

/// アセット名が配布物種別に一致するか判定する
///
/// # Arguments
/// * `asset` - リリースアセット
/// * `kind` - MSI / ポータブル
///
/// # Returns
/// * `bool` - 種別が一致すれば `true`
pub(super) fn asset_matches_kind(asset: &VersionApiAsset, kind: UpdatePackageKind) -> bool {
    match kind {
        UpdatePackageKind::Msi => Path::new(&asset.name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("msi")),
        UpdatePackageKind::PortableZip => {
            asset.name.to_ascii_lowercase().ends_with("-portable.zip")
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::super::types::{UpdatePackageKind, VersionApiAsset};
    use super::select_asset;

    fn asset(name: &str, size: u64) -> VersionApiAsset {
        VersionApiAsset {
            name: name.to_string(),
            size,
            digest: Some(format!("sha256:{}", "a".repeat(64))),
            sha256: None,
        }
    }

    /// ポータブル向けに `-portable.zip` を選ぶようにすること
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

    /// アーキテクチャが一致するアセットを選ぶようにすること
    #[test]
    fn select_asset_requires_matching_arch() {
        let assets = [
            asset("clip-refiner-0.13.1-aarch64.msi", 1),
            asset("clip-refiner-0.13.1-x86_64.msi", 2),
        ];
        let selected = select_asset(&assets, UpdatePackageKind::Msi, "x86_64").expect("msi");
        assert_eq!(selected.name, "clip-refiner-0.13.1-x86_64.msi");
    }

    /// アーキテクチャ不一致時は先頭候補へフォールバックしないようにすること
    #[test]
    fn select_asset_returns_none_when_arch_mismatches() {
        let assets = [asset("clip-refiner-0.13.1-aarch64.msi", 1)];
        assert!(select_asset(&assets, UpdatePackageKind::Msi, "x86_64").is_none());
    }

    /// `.MSI` など大文字拡張子も選ぶようにすること
    #[test]
    fn select_asset_accepts_uppercase_msi_extension() {
        let assets = [asset("clip-refiner-0.13.1-x86_64.MSI", 3)];
        let selected = select_asset(&assets, UpdatePackageKind::Msi, "x86_64").expect("msi");
        assert_eq!(selected.name, "clip-refiner-0.13.1-x86_64.MSI");
    }
}
