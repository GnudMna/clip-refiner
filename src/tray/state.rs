use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicU64};

use crate::config::{AppConfig, MonitorMode};
use crate::refiner::RefineMode;

use tao::event_loop::EventLoopProxy;

// ======================================================================
// カスタムイベント
// ======================================================================
/// アプリケーション内で発生するカスタムユーザーイベント
#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    /// クリップボード加工モードの変更要求
    RequestModeChange(RefineMode),
    /// モード選択用セレクタウィンドウの非表示要求
    HideSelector,
    /// 履歴メニューの内容再構築要求
    RefreshHistory,
    /// システム全体から受信したグローバルホットキーイベント
    Hotkey(global_hotkey::GlobalHotKeyEvent),
}

/// 履歴の最大保持数
pub const HISTORY_LIMIT: usize = 10;

/// 監視ループがループ先頭で一括取得する設定スナップショット
///
/// 1ループあたり `config` RwLock の取得を1回に削減するために使用します。
pub struct MonitorSnapshot {
    /// 現在の加工モード
    pub mode: RefineMode,
    /// ポーリング間隔（ミリ秒）
    pub interval_ms: u64,
    /// 一時停止中かどうか
    pub is_paused: bool,
    /// クリップボード履歴が有効かどうか
    pub history_enabled: bool,
}

// ======================================================================
// ロック拡張
// ======================================================================
/// `Mutex` のポイズニング（パニックによる汚染）を無視して強制的にロックを取得するための拡張
pub trait LockExt<T> {
    /// ロックを取得する。ポイズニングされている場合は汚染された状態のままデータを取得します。
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// `RwLock` のポイズニングを無視して強制的にロックを取得するための拡張
pub trait RwLockExt<T> {
    /// 読み取りロックを取得する
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T>;
    /// 書き込みロックを取得する
    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| e.into_inner())
    }
}

// ======================================================================
// アプリケーション状態
// ======================================================================
/// アプリケーション全体で共有され、スレッド間で安全に読み書きされる状態管理構造体
///
/// 設定、クリップボードの最終処理内容、履歴などを保持します。
pub struct AppState {
    /// 永続化設定データ
    pub config: RwLock<AppConfig>,
    /// クリップボード監視スレッドの世代管理カウンタ
    pub monitor_generation: AtomicU64,
    /// 二重加工防止用の、直近の処理テキスト
    pub last_processed_text: Mutex<String>,
    /// クリップボードの履歴リスト
    pub history: Mutex<Vec<String>>,
    /// メインのイベントループへメッセージを送るためのプロキシ
    pub proxy: EventLoopProxy<AppEvent>,
}

// ======================================================================
// 初期化・設定保存
// ======================================================================
impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Returns
    /// * `Self` - 新しく生成された `AppState` インスタンス。
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> Self {
        let config = AppConfig::load();
        Self {
            config: RwLock::new(config),
            monitor_generation: AtomicU64::new(0),
            last_processed_text: Mutex::new(String::new()),
            history: Mutex::new(Vec::new()),
            proxy,
        }
    }

    /// 現在の設定をファイルへ保存する。
    pub fn save_config(&self) {
        let config = self.config.read_ignore_poison();
        if let Err(e) = config.save() {
            crate::log_error!("設定の保存に失敗: {:?}", e);
        }
    }
}

// ======================================================================
// 設定操作
// ======================================================================
impl AppState {
    /// 現在の `RefineMode` をスレッドセーフに取得する。
    pub fn get_mode(&self) -> RefineMode {
        self.config.read_ignore_poison().mode
    }

    /// `RefineMode` をスレッドセーフに設定する。
    pub fn set_mode(&self, mode: RefineMode) {
        self.config.write_ignore_poison().mode = mode;
    }

    /// 現在の `MonitorMode` をスレッドセーフに取得する。
    pub fn get_monitor_mode(&self) -> MonitorMode {
        self.config.read_ignore_poison().monitor_mode
    }

    /// `MonitorMode` をスレッドセーフに設定する。
    pub fn set_monitor_mode(&self, mode: MonitorMode) {
        self.config.write_ignore_poison().monitor_mode = mode;
    }

    pub fn is_paused(&self) -> bool {
        self.config.read_ignore_poison().is_paused
    }

    pub fn set_paused(&self, paused: bool) {
        self.config.write_ignore_poison().is_paused = paused;
    }

    pub fn interval_ms(&self) -> u64 {
        self.config.read_ignore_poison().interval_ms
    }

    pub fn set_interval_ms(&self, ms: u64) {
        self.config.write_ignore_poison().interval_ms = ms;
    }

    pub fn is_history_enabled(&self) -> bool {
        self.config.read_ignore_poison().history_enabled
    }

    pub fn set_history_enabled(&self, enabled: bool) {
        self.config.write_ignore_poison().history_enabled = enabled;
    }

    pub fn is_notification_enabled(&self) -> bool {
        self.config
            .read_ignore_poison()
            .notification_settings
            .enabled
    }

    pub fn set_notification_enabled(&self, show: bool) {
        self.config
            .write_ignore_poison()
            .notification_settings
            .enabled = show;
    }

    pub fn notify_mode(&self) -> bool {
        self.config
            .read_ignore_poison()
            .notification_settings
            .notify_mode
    }

    pub fn set_notify_mode(&self, b: bool) {
        self.config
            .write_ignore_poison()
            .notification_settings
            .notify_mode = b;
    }

    pub fn notify_result(&self) -> bool {
        self.config
            .read_ignore_poison()
            .notification_settings
            .notify_result
    }

    pub fn set_notify_result(&self, b: bool) {
        self.config
            .write_ignore_poison()
            .notification_settings
            .notify_result = b;
    }

    pub fn notify_pause(&self) -> bool {
        self.config
            .read_ignore_poison()
            .notification_settings
            .notify_pause
    }

    pub fn set_notify_pause(&self, b: bool) {
        self.config
            .write_ignore_poison()
            .notification_settings
            .notify_pause = b;
    }
}

// ======================================================================
// 状態操作
// ======================================================================
impl AppState {
    /// 監視ループ向けに設定フィールドをまとめて一括取得する
    ///
    /// `config` RwLock の取得を1回に抑えることで、ループ毎の細粒度ロックを削減します。
    pub fn monitor_snapshot(&self) -> MonitorSnapshot {
        let config = self.config.read_ignore_poison();
        MonitorSnapshot {
            mode: config.mode,
            interval_ms: config.interval_ms,
            is_paused: config.is_paused,
            history_enabled: config.history_enabled,
        }
    }

    /// 加工済みの最新テキストをスレッド安全に取得する
    ///
    /// # Returns
    /// * `String` - 最後に加工されたテキストのクローン。
    pub fn get_last_processed_text(&self) -> String {
        self.last_processed_text.lock_ignore_poison().clone()
    }

    /// 加工済みの最新テキストをスレッド安全に更新する
    ///
    /// # Arguments
    /// * `text` - 新しく設定する、加工済みのテキスト。
    pub fn set_last_processed_text(&self, text: String) {
        *self.last_processed_text.lock_ignore_poison() = text;
    }

    /// 履歴を取得する
    pub fn get_history(&self) -> Vec<String> {
        self.history.lock_ignore_poison().clone()
    }

    /// 履歴をクリアする
    pub fn clear_history(&self) {
        self.history.lock_ignore_poison().clear();
    }

    /// クリップボード履歴に新しいテキストを追加する
    ///
    /// 空文字やトリム後に空になる文字列は無視されます。
    /// 既にある文字列はリストの先頭に移動し、最大保持数（ `HISTORY_LIMIT` ）を超えた分は削除されます。
    /// 追加完了後、UIスレッドへ履歴更新イベントを通知します。
    pub fn add_to_history(&self, text: String) {
        if text.trim().is_empty() {
            return;
        }

        let mut history = self.history.lock_ignore_poison();

        // 二重登録防止: すでに存在すれば削除して最上位へ
        if let Some(pos) = history.iter().position(|x| x == &text) {
            history.remove(pos);
        }

        history.insert(0, text);

        // 最大10件
        if history.len() > HISTORY_LIMIT {
            history.truncate(HISTORY_LIMIT);
        }

        let _ = self.proxy.send_event(AppEvent::RefreshHistory);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use tao::event_loop::EventLoopBuilder;
    #[cfg(windows)]
    use tao::platform::windows::EventLoopBuilderExtWindows;

    fn create_test_state() -> AppState {
        #[cfg(windows)]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        #[cfg(not(windows))]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

        AppState {
            config: RwLock::new(AppConfig {
                mode: RefineMode::Trim,
                is_paused: false,
                monitor_mode: MonitorMode::Polling,
                interval_ms: 1000,
                history_enabled: false,
                notification_settings: crate::config::NotificationSettings {
                    enabled: false,
                    notify_mode: true,
                    notify_result: true,
                    notify_pause: true,
                },
            }),
            monitor_generation: AtomicU64::new(0),
            last_processed_text: Mutex::new(String::new()),
            history: Mutex::new(Vec::new()),
            proxy: event_loop.create_proxy(),
        }
    }

    #[test]
    fn test_app_state_helpers() {
        let state = create_test_state();

        assert_eq!(state.get_mode(), RefineMode::Trim);
        state.set_mode(RefineMode::UrlEncode);
        assert_eq!(state.get_mode(), RefineMode::UrlEncode);

        assert_eq!(state.get_last_processed_text(), "");
        state.set_last_processed_text("hello".to_string());
        assert_eq!(state.get_last_processed_text(), "hello");

        assert_eq!(state.get_monitor_mode(), MonitorMode::Polling);

        state.set_interval_ms(2000);
        assert_eq!(state.interval_ms(), 2000);

        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 一時停止フラグの getter/setter
    #[test]
    fn test_paused_accessor() {
        let state = create_test_state();
        assert!(!state.is_paused());
        state.set_paused(true);
        assert!(state.is_paused());
        state.set_paused(false);
        assert!(!state.is_paused());
    }

    /// 履歴機能の getter/setter
    #[test]
    fn test_history_enabled_accessor() {
        let state = create_test_state();
        assert!(!state.is_history_enabled());
        state.set_history_enabled(true);
        assert!(state.is_history_enabled());
    }

    /// 通知関連フラグの getter/setter すべて
    #[test]
    fn test_notification_flags_accessors() {
        let state = create_test_state();

        assert!(!state.is_notification_enabled());
        state.set_notification_enabled(true);
        assert!(state.is_notification_enabled());

        assert!(state.notify_mode());
        state.set_notify_mode(false);
        assert!(!state.notify_mode());

        assert!(state.notify_result());
        state.set_notify_result(false);
        assert!(!state.notify_result());

        assert!(state.notify_pause());
        state.set_notify_pause(false);
        assert!(!state.notify_pause());
    }

    /// `monitor_snapshot` が設定値を正しく反映すること
    #[test]
    fn test_monitor_snapshot_values() {
        let state = create_test_state();
        state.set_mode(RefineMode::UrlEncode);
        state.set_interval_ms(1500);
        state.set_paused(true);
        state.set_history_enabled(true);

        let snap = state.monitor_snapshot();
        assert_eq!(snap.mode, RefineMode::UrlEncode);
        assert_eq!(snap.interval_ms, 1500);
        assert!(snap.is_paused);
        assert!(snap.history_enabled);
    }

    /// 履歴追加: 空白は無視、重複は先頭移動、上限超過分は削除、clear で空になる
    #[test]
    fn test_history_add_dedup_limit_and_clear() {
        let state = create_test_state();

        // 空白は無視
        state.add_to_history("   ".to_string());
        assert!(state.get_history().is_empty());

        // 重複するエントリは先頭に移動する
        state.add_to_history("first".to_string());
        state.add_to_history("second".to_string());
        state.add_to_history("first".to_string());
        let h = state.get_history();
        assert_eq!(h[0], "first");
        assert_eq!(h[1], "second");
        assert_eq!(h.len(), 2);

        // HISTORY_LIMIT を超えた分は切り捨てられる
        for i in 0..(HISTORY_LIMIT + 5) {
            state.add_to_history(format!("item-{i}"));
        }
        assert_eq!(state.get_history().len(), HISTORY_LIMIT);
        assert_eq!(
            state.get_history()[0],
            format!("item-{}", HISTORY_LIMIT + 4)
        );

        // clear で履歴が消える
        state.clear_history();
        assert!(state.get_history().is_empty());
    }
}
