use crate::consts;

use serde::{Deserialize, Serialize};

// ======================================================================
// 通知設定
// ======================================================================
/// 通知の内容に関する設定
///
/// どのタイミングでどのような通知を表示するかを制御する。エラー通知は `enabled` に関わらず常に表示する
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// デスクトップ通知の有効/無効 (エラー通知は常に表示)
    #[serde(default)]
    pub enabled: bool,
    /// 加工完了時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(serialize = "notifyProcess", deserialize = "notify_process"),
        alias = "notifyProcess"
    )]
    pub notify_process: bool,
    /// 一時停止切替時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(serialize = "notifyPause", deserialize = "notify_pause"),
        alias = "notifyPause"
    )]
    pub notify_pause: bool,
    /// 履歴復元時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(
            serialize = "notifyHistoryRestore",
            deserialize = "notify_history_restore"
        ),
        alias = "notifyHistoryRestore"
    )]
    pub notify_history_restore: bool,
    /// OCR 完了時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(serialize = "notifyOcr", deserialize = "notify_ocr"),
        alias = "notifyOcr"
    )]
    pub notify_ocr: bool,
    /// 登録クリップ操作時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(
            serialize = "notifyRegisteredClip",
            deserialize = "notify_registered_clip"
        ),
        alias = "notifyRegisteredClip"
    )]
    pub notify_registered_clip: bool,
    /// お気に入り操作時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(serialize = "notifyFavorites", deserialize = "notify_favorites"),
        alias = "notifyFavorites"
    )]
    pub notify_favorites: bool,
    /// 連続記録操作時に通知するかどうか
    #[serde(
        default = "consts::default_true",
        rename(
            serialize = "notifyContinuousRecord",
            deserialize = "notify_continuous_record"
        ),
        alias = "notifyContinuousRecord"
    )]
    pub notify_continuous_record: bool,
    /// 通知にクリップボードの内容を含めるかどうか
    #[serde(
        default,
        rename(serialize = "notifyResult", deserialize = "notify_result"),
        alias = "notifyResult"
    )]
    pub notify_result: bool,
    /// 旧設定 `notify_mode` の読み込み用 (保存時は出力しない)
    #[serde(
        default = "consts::default_true",
        rename(serialize = "notifyMode", deserialize = "notify_mode"),
        alias = "notifyMode",
        skip_serializing
    )]
    legacy_notify_mode: bool,
    /// 旧設定 `notify_actions` の読み込み用 (保存時は出力しない)
    #[serde(
        default = "consts::default_true",
        rename(serialize = "notifyActions", deserialize = "notify_actions"),
        alias = "notifyActions",
        skip_serializing
    )]
    legacy_notify_actions: bool,
}

impl NotificationSettings {
    /// 旧設定項目を新しい個別トグルへ移行する
    pub fn migrate_legacy_fields(&mut self) {
        if !self.legacy_notify_mode && !self.notify_result {
            self.notify_process = false;
        }
        if !self.legacy_notify_actions {
            self.notify_history_restore = false;
            self.notify_ocr = false;
            self.notify_registered_clip = false;
            self.notify_favorites = false;
            self.notify_continuous_record = false;
        }
    }
}

impl Default for NotificationSettings {
    /// デフォルトの通知設定を生成する
    ///
    /// # Returns
    /// * `Self` - 通知オフ・内容表示オフ・その他サブ設定はオンのデフォルト設定
    fn default() -> Self {
        Self {
            enabled: false,
            notify_process: true,
            notify_pause: true,
            notify_history_restore: true,
            notify_ocr: true,
            notify_registered_clip: true,
            notify_favorites: true,
            notify_continuous_record: true,
            notify_result: false,
            legacy_notify_mode: true,
            legacy_notify_actions: true,
        }
    }
}
