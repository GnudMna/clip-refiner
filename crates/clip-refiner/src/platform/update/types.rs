use super::error::UpdateError;

use serde::{Deserialize, Serialize};

// ======================================================================
// リリース照会
// ======================================================================
/// gh-release-bridge の version API レスポンス
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VersionApiResponse {
    /// リリースタグ (例: `v0.13.1`)
    pub tag: String,
    /// 許可されたアセット一覧
    #[serde(default)]
    pub assets: Vec<VersionApiAsset>,
}

/// version API のアセット要素
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VersionApiAsset {
    /// アセットファイル名
    pub name: String,
    /// バイトサイズ
    #[serde(default)]
    pub size: u64,
    /// GitHub Release Asset の digest (例: `sha256:<hex>`)
    #[serde(default)]
    pub digest: Option<String>,
    /// digest の代替フィールド (素の16進も可)
    #[serde(default)]
    pub sha256: Option<String>,
}

impl VersionApiAsset {
    /// 完全性検証に使う生文字列を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<&str>` - 値がある場合は `Some`、ない場合は `None`
    pub fn integrity_raw(&self) -> Option<&str> {
        self.digest
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                self.sha256
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
    }
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
    /// 現在の実行形態から配布物種別を返す
    ///
    /// # Returns
    /// * `Self` - ポータブルなら `PortableZip`、それ以外は `Msi`
    pub fn current() -> Self {
        if crate::config::is_portable_mode() {
            Self::PortableZip
        } else {
            Self::Msi
        }
    }
}

/// 適用可能な更新アセット
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// 設定 UI / IPC へ返す更新確認レスポンス
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub(crate) enum UpdateCheckUiResponse {
    /// 最新版
    #[serde(rename_all = "camelCase")]
    UpToDate {
        /// 現在のバージョン
        current_version: String,
        /// リモートの最新バージョン
        latest_version: String,
    },
    /// 更新あり
    #[serde(rename_all = "camelCase")]
    Available {
        /// 現在のバージョン
        current_version: String,
        /// 最新バージョン
        latest_version: String,
        /// リリースタグ
        tag: String,
        /// アセット名
        asset_name: String,
        /// サイズ (バイト)
        size: u64,
    },
    /// 失敗
    #[serde(rename_all = "camelCase")]
    Error {
        /// ユーザー向けメッセージ
        message: String,
    },
    /// 別の更新操作が進行中
    Busy,
}

impl From<&UpdateCheckOutcome> for UpdateCheckUiResponse {
    fn from(outcome: &UpdateCheckOutcome) -> Self {
        match outcome {
            UpdateCheckOutcome::UpToDate {
                current_version,
                latest_version,
            } => Self::UpToDate {
                current_version: current_version.clone(),
                latest_version: latest_version.clone(),
            },
            UpdateCheckOutcome::Available(update) => Self::Available {
                current_version: update.current_version.clone(),
                latest_version: update.latest_version.clone(),
                tag: update.tag.clone(),
                asset_name: update.asset_name.clone(),
                size: update.size,
            },
            UpdateCheckOutcome::Error { message } => Self::Error {
                message: message.clone(),
            },
        }
    }
}

/// 更新適用の結果
#[derive(Debug, Clone)]
pub enum UpdateApplyOutcome {
    /// MSI インストーラーを起動した (アプリ終了が必要)
    MsiStarted,
    /// ポータブル ZIP を展開した
    PortableExtracted {
        /// 展開先ディレクトリ
        path: String,
        /// 差し替えヘルパーを起動したか (起動済みならアプリ終了が必要)
        helper_launched: bool,
    },
    /// 適用に失敗
    Error {
        /// ユーザー向けメッセージ (`UpdateError` の `Display`)
        message: String,
    },
}

impl UpdateApplyOutcome {
    /// 型付きエラーから適用失敗 Outcome を組み立てる
    ///
    /// # Arguments
    /// * `err` - 更新適用の失敗理由
    ///
    /// # Returns
    /// * `Self` - `Error { message }`
    pub(crate) fn from_error(err: &UpdateError) -> Self {
        Self::Error {
            message: err.to_string(),
        }
    }
}

impl From<UpdateError> for UpdateApplyOutcome {
    fn from(err: UpdateError) -> Self {
        Self::from_error(&err)
    }
}

impl From<&UpdateError> for UpdateApplyOutcome {
    fn from(err: &UpdateError) -> Self {
        Self::from_error(err)
    }
}

/// 設定 UI へ返す更新適用レスポンス
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub(crate) enum UpdateApplyUiResponse {
    /// MSI を起動した
    #[serde(rename_all = "camelCase")]
    MsiStarted {
        /// 案内メッセージ
        message: String,
    },
    /// ポータブルを展開した
    #[serde(rename_all = "camelCase")]
    PortableExtracted {
        /// 展開先パス
        path: String,
        /// 案内メッセージ
        message: String,
    },
    /// 失敗
    #[serde(rename_all = "camelCase")]
    Error {
        /// ユーザー向けメッセージ
        message: String,
    },
    /// 適用対象が無い / 進行中
    #[serde(rename_all = "camelCase")]
    Busy {
        /// ユーザー向けメッセージ
        message: String,
    },
}

impl From<&UpdateApplyOutcome> for UpdateApplyUiResponse {
    fn from(outcome: &UpdateApplyOutcome) -> Self {
        match outcome {
            UpdateApplyOutcome::MsiStarted => Self::MsiStarted {
                message: "インストーラーを起動しました。アプリを終了します".to_string(),
            },
            UpdateApplyOutcome::PortableExtracted {
                path,
                helper_launched: true,
            } => Self::PortableExtracted {
                path: path.clone(),
                message: "差し替えを開始しました。アプリを終了し、新しい版を自動で起動します"
                    .to_string(),
            },
            UpdateApplyOutcome::PortableExtracted {
                path,
                helper_launched: false,
            } => Self::PortableExtracted {
                path: path.clone(),
                message: format!(
                    "展開しました。アプリを終了し、次のフォルダの内容で実行フォルダを差し替えて再起動してください: {path}"
                ),
            },
            UpdateApplyOutcome::Error { message } => Self::Error {
                message: message.clone(),
            },
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 更新確認 UI レスポンスが kebab-case / camelCase でシリアライズされるようにすること
    #[test]
    fn update_check_ui_response_serializes_for_js() {
        let available = UpdateCheckUiResponse::Available {
            current_version: "0.13.0".to_string(),
            latest_version: "0.13.1".to_string(),
            tag: "v0.13.1".to_string(),
            asset_name: "clip-refiner-0.13.1-x86_64.msi".to_string(),
            size: 10,
        };
        let json = serde_json::to_value(&available).expect("serialize");
        assert_eq!(json["status"], "available");
        assert_eq!(json["latestVersion"], "0.13.1");
        assert_eq!(json["assetName"], "clip-refiner-0.13.1-x86_64.msi");
        assert!(json.get("latest_version").is_none());

        let busy = serde_json::to_value(&UpdateCheckUiResponse::Busy).expect("busy");
        assert_eq!(busy["status"], "busy");
    }

    /// 更新適用 UI レスポンスが設定 UI 向け JSON になるようにすること
    #[test]
    fn update_apply_ui_response_serializes_for_js() {
        let portable = UpdateApplyUiResponse::from(&UpdateApplyOutcome::PortableExtracted {
            path: r"C:\Temp\out".to_string(),
            helper_launched: true,
        });
        let json = serde_json::to_value(&portable).expect("serialize");
        assert_eq!(json["status"], "portable-extracted");
        assert_eq!(json["path"], r"C:\Temp\out");
        assert!(json["message"].as_str().unwrap().contains("差し替え"));

        let manual = UpdateApplyUiResponse::from(&UpdateApplyOutcome::PortableExtracted {
            path: r"C:\Temp\out".to_string(),
            helper_launched: false,
        });
        let json = serde_json::to_value(&manual).expect("manual");
        assert!(json["message"].as_str().unwrap().contains(r"C:\Temp\out"));

        let msi = UpdateApplyUiResponse::from(&UpdateApplyOutcome::MsiStarted);
        let json = serde_json::to_value(&msi).expect("msi");
        assert_eq!(json["status"], "msi-started");
    }

    /// `UpdateCheckOutcome` から UI レスポンスへ変換できるようにすること
    #[test]
    fn update_check_outcome_maps_to_ui_response() {
        let outcome = UpdateCheckOutcome::UpToDate {
            current_version: "0.13.1".to_string(),
            latest_version: "0.13.1".to_string(),
        };
        let ui = UpdateCheckUiResponse::from(&outcome);
        assert!(matches!(ui, UpdateCheckUiResponse::UpToDate { .. }));
    }
}
