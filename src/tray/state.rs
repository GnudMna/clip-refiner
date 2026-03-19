use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicU64};

use crate::config::{AppConfig, MonitorMode};
use crate::refiner::RefineMode;

use tao::event_loop::EventLoopProxy;

/// アプリケーション内でのカスタムイベント
#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    /// モード変更要求
    RequestModeChange(RefineMode),
    /// セレクターを閉じる
    HideSelector,
    /// 履歴メニューの更新要求
    RefreshHistory,
    /// ホットキーイベント
    Hotkey(global_hotkey::GlobalHotKeyEvent),
}

/// 履歴の最大保持数
pub const HISTORY_LIMIT: usize = 10;

/// Mutexのポイズニングを無視してロックを取得するための拡張トレイト
pub trait LockExt<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub trait RwLockExt<T> {
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T>;
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

/// アプリケーション内で共有されるミュータブルな状態
///
/// Mutexのロックに失敗した場合（ポイズニング）、パニックせずに以前の値を返して
/// アプリケーションの実行を継続する方針をとる。
pub struct AppState {
    /// 永続化される設定全体を1つのRwLock内で管理
    pub config: RwLock<AppConfig>,
    /// 監視スレッドの世代管理用カウンタ。設定変更時に古いスレッドを破棄するために使用
    pub monitor_generation: AtomicU64,
    /// 二重加工を防止するために保持される、最後に加工されたテキスト
    pub last_processed_text: Mutex<String>,
    /// クリップボード履歴（最大10件）
    pub history: Mutex<Vec<String>>,
    /// イベントループへのプロキシ。別スレッドからUIイベント（例: 履歴更新）を送信するために使用される。
    pub proxy: EventLoopProxy<AppEvent>,
}

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
            eprintln!("設定の保存に失敗: {}", e);
        }
    }

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

    pub fn show_success_notification(&self) -> bool {
        self.config.read_ignore_poison().show_success_notification
    }

    pub fn set_show_success_notification(&self, show: bool) {
        self.config.write_ignore_poison().show_success_notification = show;
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

    /// 履歴にテキストを追加する。
    /// すでに存在する場合は最上位に移動させ、最大10件を保持する。
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

#[cfg(test)]
mod tests {
    use super::*;
    use tao::event_loop::EventLoopBuilder;
    #[cfg(windows)]
    use tao::platform::windows::EventLoopBuilderExtWindows;

    #[test]
    fn test_app_state_helpers() {
        #[cfg(windows)]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        #[cfg(not(windows))]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

        let state = AppState {
            config: RwLock::new(AppConfig {
                mode: RefineMode::Trim,
                is_paused: false,
                monitor_mode: MonitorMode::Polling,
                interval_ms: 1000,
                history_enabled: false,
                show_success_notification: false,
                notification_settings: crate::config::NotificationSettings {
                    notify_mode: true,
                    notify_result: true,
                    notify_pause: true,
                },
            }),
            monitor_generation: AtomicU64::new(0),
            last_processed_text: Mutex::new(String::new()),
            history: Mutex::new(Vec::new()),
            proxy: event_loop.create_proxy(),
        };

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
}
