use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicU64};

use crate::config::AppConfig;
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

/// 監視ループにおける二重加工防止用の状態
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProcessedState {
    /// ポーリングで前回観測したクリップボード本文
    pub last_seen_text: String,
    /// 直近の加工でクリップボードへ書き戻した本文（自身の変更イベントを1回無視）
    pub last_written_text: Option<String>,
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
    /// 二重加工防止用の監視状態
    processed_state: Mutex<ProcessedState>,
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
            processed_state: Mutex::new(ProcessedState::default()),
            history: Mutex::new(Vec::new()),
            proxy,
        }
    }

    /// 現在の設定をファイルへ保存する。
    pub fn save_config(&self) {
        if let Err(e) = self.with_config(|c| c.save()) {
            crate::log_error!("設定の保存に失敗: {:?}", e);
        }
    }

    /// 設定を読み取り専用で参照する
    pub fn with_config<R>(&self, f: impl FnOnce(&AppConfig) -> R) -> R {
        f(&self.config.read_ignore_poison())
    }

    /// 設定を変更する
    pub fn with_config_mut<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> R {
        f(&mut self.config.write_ignore_poison())
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
        self.with_config(|config| MonitorSnapshot {
            mode: config.mode,
            interval_ms: config.interval_ms,
            is_paused: config.is_paused,
            history_enabled: config.history_enabled,
        })
    }

    /// 二重加工防止状態を更新する
    pub fn with_processed_state<R>(&self, f: impl FnOnce(&mut ProcessedState) -> R) -> R {
        f(&mut self.processed_state.lock_ignore_poison())
    }

    /// 加工成功後にクリップボードへ書き戻したことを記録する
    pub fn record_processing_success(&self, output: &str) {
        self.with_processed_state(|ps| {
            ps.last_written_text = Some(output.to_string());
            ps.last_seen_text = output.to_string();
        });
    }

    /// 加工せずに観測したクリップボード本文を記録する
    pub fn record_clipboard_observed(&self, text: &str) {
        self.with_processed_state(|ps| {
            ps.last_seen_text = text.to_string();
        });
    }

    /// 履歴復元など、外部からクリップボードへ設定した本文を記録する
    pub fn record_clipboard_set(&self, text: &str) {
        self.with_processed_state(|ps| {
            ps.last_written_text = None;
            ps.last_seen_text = text.to_string();
        });
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
            processed_state: Mutex::new(ProcessedState::default()),
            history: Mutex::new(Vec::new()),
            proxy: event_loop.create_proxy(),
        }
    }

    #[test]
    fn test_app_state_helpers() {
        let state = create_test_state();

        assert_eq!(state.with_config(|c| c.mode), RefineMode::Trim);
        state.with_config_mut(|c| c.mode = RefineMode::UrlEncode);
        assert_eq!(state.with_config(|c| c.mode), RefineMode::UrlEncode);

        let mut ps = ProcessedState::default();
        ps.last_seen_text = "hello".to_string();
        state.with_processed_state(|s| *s = ps.clone());
        assert_eq!(
            state.with_processed_state(|s| s.last_seen_text.clone()),
            "hello"
        );

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Polling);

        state.with_config_mut(|c| c.interval_ms = 2000);
        assert_eq!(state.with_config(|c| c.interval_ms), 2000);

        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 一時停止フラグの更新
    #[test]
    fn test_paused_accessor() {
        let state = create_test_state();
        assert!(!state.with_config(|c| c.is_paused));
        state.with_config_mut(|c| c.is_paused = true);
        assert!(state.with_config(|c| c.is_paused));
        state.with_config_mut(|c| c.is_paused = false);
        assert!(!state.with_config(|c| c.is_paused));
    }

    /// 履歴機能の更新
    #[test]
    fn test_history_enabled_accessor() {
        let state = create_test_state();
        assert!(!state.with_config(|c| c.history_enabled));
        state.with_config_mut(|c| c.history_enabled = true);
        assert!(state.with_config(|c| c.history_enabled));
    }

    /// 通知設定の更新
    #[test]
    fn test_notification_settings_accessor() {
        let state = create_test_state();

        assert!(!state.with_config(|c| c.notification_settings.enabled));
        state.with_config_mut(|c| c.notification_settings.enabled = true);
        assert!(state.with_config(|c| c.notification_settings.enabled));

        assert!(state.with_config(|c| c.notification_settings.notify_mode));
        state.with_config_mut(|c| c.notification_settings.notify_mode = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_mode));

        assert!(state.with_config(|c| c.notification_settings.notify_result));
        state.with_config_mut(|c| c.notification_settings.notify_result = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_result));

        assert!(state.with_config(|c| c.notification_settings.notify_pause));
        state.with_config_mut(|c| c.notification_settings.notify_pause = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_pause));
    }

    /// `monitor_snapshot` が設定値を正しく反映すること
    #[test]
    fn test_monitor_snapshot_values() {
        let state = create_test_state();
        state.with_config_mut(|c| {
            c.mode = RefineMode::UrlEncode;
            c.interval_ms = 1500;
            c.is_paused = true;
            c.history_enabled = true;
        });

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
