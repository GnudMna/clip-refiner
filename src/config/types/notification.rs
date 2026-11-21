use crate::consts;

use serde::{Deserialize, Serialize};

// ======================================================================
// 通知設定
// ======================================================================
/// 通知の内容に関する設定
///
/// どのタイミングでどのような通知を表示するかを制御する
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 成功通知機能全体の有効/無効スイッチ
    #[serde(default)]
    pub enabled: bool,
    /// 実行されたモード名を通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_mode: bool,
    /// 通知にクリップボードの内容 (加工結果) を含めるかどうか
    #[serde(default)]
    pub notify_result: bool,
    /// 一時停止の切り替えを通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_pause: bool,
}

impl Default for NotificationSettings {
    /// デフォルトの通知設定を生成する
    ///
    /// # Returns
    /// * `Self` - 通知オフ・内容表示オフ・その他サブ設定はオンのデフォルト設定
    fn default() -> Self {
        Self {
            enabled: false,
            notify_mode: true,
            notify_result: false,
            notify_pause: true,
        }
    }
}
