//! 設定読み込み・再読み込みの結果型

use super::super::types::AppConfig;

// ======================================================================
// 読み込み結果
// ======================================================================
/// 設定ファイルの再読み込みエラー
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigReloadError {
    /// 設定ファイルパスの取得に失敗
    #[error("設定ファイルの場所を取得できませんでした: {0}")]
    Path(String),
    /// 設定ファイルが存在しない
    #[error("設定ファイルが見つかりません")]
    NotFound,
    /// ファイル読み取りに失敗
    #[error("設定ファイルを読み取れませんでした: {0}")]
    Read(String),
    /// TOML 解析に失敗
    #[error("設定ファイルの形式が不正です: {0}")]
    Parse(String),
}

impl ConfigReloadError {
    /// ユーザー向けのエラーメッセージを返す
    ///
    /// # Arguments
    /// * `self` - 再読み込みエラー
    ///
    /// # Returns
    /// * `&str` - UI 表示用の固定メッセージ
    #[must_use]
    pub fn user_message(&self) -> &str {
        match self {
            Self::Path(_) => "設定ファイルの場所を取得できませんでした",
            Self::NotFound => "設定ファイルが見つかりません",
            Self::Read(_) => "設定ファイルを読み取れませんでした",
            Self::Parse(_) => "設定ファイルの形式が不正です",
        }
    }

    /// デスクトップ通知向けのメッセージを返す
    ///
    /// `Parse` のみ詳細を括弧付きで付与し、それ以外は `user_message` と同じ
    ///
    /// # Arguments
    /// * `self` - 再読み込みエラー
    ///
    /// # Returns
    /// * `String` - 通知本文
    #[must_use]
    pub fn notification_message(&self) -> String {
        match self {
            Self::Parse(detail) => format!("{} ({detail})", self.user_message()),
            _ => self.user_message().to_string(),
        }
    }
}

/// 設定読み込みの結果
#[derive(Debug, Clone)]
pub struct ConfigLoadOutcome {
    /// 読み込んだ設定 (失敗時はデフォルト)
    pub config: AppConfig,
    /// デフォルトへフォールバックした場合の理由
    pub fallback: Option<ConfigLoadFallback>,
    /// 認識できない設定キー (解析成功時のみ。読み込み自体は継続)
    pub unknown_keys: Vec<String>,
    /// 移行手順が無い `version` を検出し、既存項目を維持した
    pub unsupported_version_kept: bool,
    /// 登録クリップの鍵を取得できず、登録クリップを読み込めなかった
    pub registered_clips_key_unavailable: bool,
}

/// ディスクからの設定再読み込み結果
#[derive(Debug, Clone)]
pub struct ConfigDiskReload {
    /// 読み込んだ設定
    pub config: AppConfig,
    /// スキーマ移行またはレガシー移行を実行したか
    pub migrated: bool,
    /// 将来の `version` を現行へ揃えたか
    pub newer_version_clamped: bool,
    /// 移行手順が無い `version` を検出し、既存項目を維持した
    pub unsupported_version_kept: bool,
    /// 認識できない設定キー (読み込み自体は継続)
    pub unknown_keys: Vec<String>,
}

/// 設定読み込み失敗時にデフォルトへフォールバックした理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoadFallback {
    /// 失敗理由
    pub error: ConfigReloadError,
    /// 解析失敗時に `config.toml.bak` を作成できたか
    pub backup_created: bool,
}

impl ConfigLoadFallback {
    /// デスクトップ通知・CLI 向けのメッセージを返す
    ///
    /// # Arguments
    /// * `self` - フォールバック理由
    ///
    /// # Returns
    /// * `String` - エラー内容とバックアップ成否を含むメッセージ
    #[must_use]
    pub fn user_message(&self) -> String {
        let core = self.error.notification_message();
        let suffix = match (&self.error, self.backup_created) {
            (ConfigReloadError::Parse(_), true) => {
                "。デフォルト設定で起動します。元ファイルは config.toml.bak に保存しました"
            }
            (ConfigReloadError::Parse(_), false) => {
                "。デフォルト設定で起動します。元ファイルのバックアップに失敗しました"
            }
            _ => "。デフォルト設定で起動します",
        };
        format!("{core}{suffix}")
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::{ConfigLoadFallback, ConfigReloadError};

    /// 通知向けメッセージが Parse 詳細のみ括弧付きになるようにすること
    #[test]
    fn config_reload_error_notification_message_includes_parse_detail() {
        let parse = ConfigReloadError::Parse("unexpected key".to_string());
        assert_eq!(
            parse.notification_message(),
            "設定ファイルの形式が不正です (unexpected key)"
        );

        let read = ConfigReloadError::Read("access denied".to_string());
        assert_eq!(
            read.notification_message(),
            "設定ファイルを読み取れませんでした"
        );
    }

    /// フォールバック理由からユーザー向けメッセージを組み立てられるようにすること
    #[test]
    fn config_load_fallback_user_message_includes_backup_hint() {
        let with_backup = ConfigLoadFallback {
            error: ConfigReloadError::Parse("unexpected key".to_string()),
            backup_created: true,
        };
        let message = with_backup.user_message();
        assert!(message.contains("形式が不正"));
        assert!(message.contains("config.toml.bak"));

        let without_backup = ConfigLoadFallback {
            error: ConfigReloadError::Read("access denied".to_string()),
            backup_created: false,
        };
        assert!(
            without_backup
                .user_message()
                .contains("読み取れませんでした")
        );
        assert!(
            without_backup
                .user_message()
                .contains("デフォルト設定で起動")
        );
    }
}
