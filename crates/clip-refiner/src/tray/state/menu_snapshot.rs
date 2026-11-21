use std::collections::HashSet;

use crate::config::{HotkeySettings, MonitorMode, NotificationSettings};
use crate::refiner::RefineMode;

// ======================================================================
// メニュースナップショット
// ======================================================================
/// トレイメニュー構築・同期向けの設定スナップショット
///
/// `AppConfig` 全体 (登録クリップ一覧を含む) を clone せず、メニュー表示に必要な
/// フィールドだけをコピーする
pub struct MenuSnapshot {
    /// 現在選択中の変換モード (カテゴリ表示用)
    pub mode: RefineMode,
    /// 有効なパイプライン上の変換モード集合
    pub active_modes: HashSet<RefineMode>,
    /// お気に入り変換モード
    pub favorite_modes: Vec<RefineMode>,
    /// ホットキー設定 (お気に入りスロットのラベル表示用)
    pub hotkeys: HotkeySettings,
    /// 監視方式
    pub monitor_mode: MonitorMode,
    /// ポーリング間隔 (ミリ秒)
    pub interval_ms: u64,
    /// 履歴機能が有効かどうか
    pub history_enabled: bool,
    /// 連続記録が有効かどうか
    pub continuous_record_enabled: bool,
    /// 通知設定
    pub notification_settings: NotificationSettings,
    /// 一時停止中かどうか
    pub is_paused: bool,
}
