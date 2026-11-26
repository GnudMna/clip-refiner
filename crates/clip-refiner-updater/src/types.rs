use std::path::{Path, PathBuf};

use clip_refiner_core::consts::PORTABLE_MARKER_FILE;
use serde::Deserialize;

use super::error::UpdateError;

// ======================================================================
// リリース照会
// ======================================================================
/// GitHub API の最新リリース (使う項目のみ)
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LatestRelease {
    /// リリースタグ (例: `v0.13.1`)
    pub tag_name: String,
    /// リリースアセット一覧
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

/// GitHub リリースのアセット要素
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseAsset {
    /// アセットファイル名
    pub name: String,
    /// バイトサイズ
    #[serde(default)]
    pub size: u64,
    /// アセットの digest (例: `sha256:<hex>`)
    #[serde(default)]
    pub digest: Option<String>,
}

/// インストール形態に応じて選ぶ配布物の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePackageKind {
    /// MSI インストーラー
    Msi,
    /// ポータブル ZIP
    PortableZip,
}

impl UpdatePackageKind {
    /// インストール先から配布物種別を返す
    ///
    /// # Arguments
    /// * `install_dir` - `ClipRefiner.exe` のあるフォルダ
    ///
    /// # Returns
    /// * `Self` - `portable` マーカーがあれば `PortableZip`、それ以外は `Msi`
    pub fn for_install_dir(install_dir: &Path) -> Self {
        if install_dir.join(PORTABLE_MARKER_FILE).is_file() {
            Self::PortableZip
        } else {
            Self::Msi
        }
    }
}

/// 適用可能な更新アセット
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    /// 現在のアプリバージョン (`SemVer`、`v` なし)
    pub current_version: String,
    /// 最新バージョン (`SemVer`、`v` なし)
    pub latest_version: String,
    /// リリースタグ (例: `v0.13.1`)
    pub tag: String,
    /// ダウンロードするアセット名
    pub asset_name: String,
    /// アセットサイズ (バイト)
    pub size: u64,
    /// 期待する SHA-256 (小文字16進 64 桁)
    pub sha256: String,
}

/// 更新確認の結果
#[derive(Debug, Clone)]
pub enum UpdateCheckOutcome {
    /// 最新版を利用中
    UpToDate {
        /// 現在のバージョン
        current_version: String,
        /// リモートの最新バージョン
        latest_version: String,
    },
    /// 新しい版が利用可能
    Available(AvailableUpdate),
    /// 確認に失敗
    Error {
        /// ユーザー向けメッセージ (`UpdateError` の `Display`)
        message: String,
    },
}

impl UpdateCheckOutcome {
    /// 型付きエラーから確認失敗 Outcome を組み立てる
    ///
    /// # Arguments
    /// * `err` - 更新確認の失敗理由
    ///
    /// # Returns
    /// * `Self` - `Error { message }`
    pub(crate) fn from_error(err: &UpdateError) -> Self {
        Self::Error {
            message: err.to_string(),
        }
    }
}

impl From<UpdateError> for UpdateCheckOutcome {
    fn from(err: UpdateError) -> Self {
        Self::from_error(&err)
    }
}

impl From<&UpdateError> for UpdateCheckOutcome {
    fn from(err: &UpdateError) -> Self {
        Self::from_error(err)
    }
}

// ======================================================================
// 適用
// ======================================================================
/// 検証を終え、インストールできる状態の配布物
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedPackage {
    /// 検証済みの MSI
    Msi {
        /// ダウンロードした MSI
        path: PathBuf,
    },
    /// 展開済みのポータブル配布物
    Portable {
        /// 展開ルート (新しい `ClipRefiner.exe` があるフォルダ)
        source_dir: PathBuf,
    },
}

/// 配布物のインストール結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallOutcome {
    /// インストールが完了した
    Installed,
    /// 完了に Windows の再起動が必要
    RebootRequired,
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// `portable` マーカーの有無で配布物種別を判定すること
    #[test]
    fn package_kind_follows_portable_marker() {
        let dir = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-kind-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        assert_eq!(
            UpdatePackageKind::for_install_dir(&dir),
            UpdatePackageKind::Msi
        );

        fs::write(dir.join(PORTABLE_MARKER_FILE), b"").expect("marker");
        assert_eq!(
            UpdatePackageKind::for_install_dir(&dir),
            UpdatePackageKind::PortableZip
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
