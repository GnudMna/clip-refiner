use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicU64};

use super::history::EncryptedHistoryStore;
use crate::config::AppConfig;
use crate::refiner::RefineMode;
use crate::security::{ContentFingerprint, SecretString, secret_from};

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

/// 監視ループがループ先頭で一括取得する設定スナップショット
///
/// 1ループあたり `config` `RwLock` の取得を1回に削減するために使用する
pub struct MonitorSnapshot {
    /// 現在の加工モード
    pub mode: RefineMode,
    /// ポーリング間隔(ミリ秒)
    pub interval_ms: u64,
    /// 一時停止中かどうか
    pub is_paused: bool,
    /// クリップボード履歴が有効かどうか
    pub history_enabled: bool,
}

/// 監視ループにおける二重加工防止用の状態
///
/// クリップボード本文は平文で保持せず、指紋のみを記録する
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProcessedState {
    /// ポーリングで前回観測したクリップボード本文の指紋
    pub last_seen: ContentFingerprint,
    /// 直近の加工で書き戻した本文の指紋 (自身の変更イベントを1回無視)
    pub last_written: Option<ContentFingerprint>,
}

impl ProcessedState {
    /// 指定テキストが `last_seen` と一致するか判定する
    pub fn matches_last_seen(&self, text: &str) -> bool {
        self.last_seen.matches(text)
    }

    /// 指定テキストが `last_written` と一致するか判定する
    pub fn matches_last_written(&self, text: &str) -> bool {
        self.last_written.is_some_and(|fp| fp.matches(text))
    }
}

// ======================================================================
// ロック拡張
// ======================================================================
/// `Mutex` のポイズニング(パニックによる汚染)を無視して強制的にロックを取得するための拡張
pub trait LockExt<T> {
    /// ロックを取得する。ポイズニングされている場合は汚染された状態のままデータを取得する。
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        self.read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T> {
        self.write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// ======================================================================
// アプリケーション状態
// ======================================================================
/// アプリケーション全体で共有され、スレッド間で安全に読み書きされる状態管理構造体
///
/// 設定、クリップボードの最終処理内容、暗号化履歴などを保持する
pub struct AppState {
    /// 永続化設定データ
    pub config: RwLock<AppConfig>,
    /// クリップボード監視スレッドの世代管理カウンタ
    pub monitor_generation: AtomicU64,
    /// 二重加工防止用の監視状態
    processed_state: Mutex<ProcessedState>,
    /// 直近の加工前テキスト (取り消し用、破棄時にゼロ化)
    undo_text: Mutex<Option<SecretString>>,
    /// 暗号化されたクリップボード履歴 (セッション限定、メモリ内のみ)
    history_store: Mutex<EncryptedHistoryStore>,
    /// メインのイベントループへメッセージを送るためのプロキシ
    pub proxy: EventLoopProxy<AppEvent>,
    /// 設定をディスクへ保存するかどうか
    persist_config: bool,
}

// ======================================================================
// 初期化・設定保存
// ======================================================================
impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Returns
    /// * `Self` - 新しく生成された `AppState` インスタンス
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> Self {
        let config = AppConfig::load();
        Self {
            config: RwLock::new(config),
            monitor_generation: AtomicU64::new(0),
            processed_state: Mutex::new(ProcessedState::default()),
            undo_text: Mutex::new(None),
            history_store: Mutex::new(EncryptedHistoryStore::new()),
            proxy,
            persist_config: true,
        }
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// `persist_config` が `false` の場合はメモリ上の変更のみとし、ディスクへは書き込まない
    pub fn save_config(&self) {
        if !self.persist_config {
            return;
        }
        if let Err(e) = self.with_config(super::super::config::AppConfig::save) {
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

    /// 設定のディスク保存が有効かどうかを返す
    #[cfg(test)]
    pub(crate) fn is_config_persistence_enabled(&self) -> bool {
        self.persist_config
    }
}

// ======================================================================
// 状態操作
// ======================================================================
impl AppState {
    /// 監視ループ向けに設定フィールドをまとめて一括取得する
    ///
    /// `config` `RwLock` の取得を1回に抑えることで、ループ毎の細粒度ロックを削減する
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
            let fp = ContentFingerprint::from_text(output);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 加工せずに観測したクリップボード本文を記録する
    pub fn record_clipboard_observed(&self, text: &str) {
        self.with_processed_state(|ps| {
            ps.last_seen = ContentFingerprint::from_text(text);
        });
    }

    /// 履歴復元など、外部からクリップボードへ設定した本文を記録する
    pub fn record_clipboard_set(&self, text: &str) {
        self.with_processed_state(|ps| {
            ps.last_written = None;
            ps.last_seen = ContentFingerprint::from_text(text);
        });
    }

    /// 加工成功時に取り消し用の元テキストを記録する
    pub fn record_undo_source(&self, text: &str) {
        *self.undo_text.lock_ignore_poison() = Some(secret_from(text));
    }

    /// 取り消し用の元テキストを取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 取り消し可能な加工があれば `Some(元テキスト)`
    pub fn take_undo_source(&self) -> Option<SecretString> {
        self.undo_text.lock_ignore_poison().take()
    }

    /// 履歴の件数を返す
    pub fn history_len(&self) -> usize {
        self.history_store.lock_ignore_poison().len()
    }

    /// 指定インデックスの履歴を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 履歴ストア内のインデックス (0 が最新)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_history_entry(&self, index: usize) -> Option<SecretString> {
        self.history_store.lock_ignore_poison().entry_at(index)
    }

    /// 履歴をクリアする
    pub fn clear_history(&self) {
        self.history_store.lock_ignore_poison().clear();
    }

    /// クリップボード履歴に新しいテキストを追加する
    ///
    /// 空文字やトリム後に空になる文字列は無視される
    /// 既にある文字列はリストの先頭に移動し、設定の `history_limit` を超えた分は削除される
    /// 本文はメモリ上で暗号化して保持し、再起動時には破棄される
    /// 追加完了後、UIスレッドへ履歴更新イベントを通知する
    ///
    /// # Arguments
    /// * `text` - 履歴へ追加するクリップボード本文
    pub fn add_to_history(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.trim().is_empty() {
            return;
        }

        let limit = self.with_config(|c| c.history_limit);
        self.history_store.lock_ignore_poison().add(text, limit);

        let _ = self.proxy.send_event(AppEvent::RefreshHistory);
    }
}

// ======================================================================
// テスト用ヘルパー
// ======================================================================
/// ユニットテスト用の `AppState` を生成する
#[cfg(any(test, feature = "test-helpers"))]
pub(crate) fn test_app_state() -> AppState {
    use crate::config::AppConfig;
    use tao::event_loop::EventLoopBuilder;
    #[cfg(windows)]
    use tao::platform::windows::EventLoopBuilderExtWindows;

    #[cfg(windows)]
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    #[cfg(not(windows))]
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

    let config = AppConfig {
        mode: RefineMode::Trim,
        ..Default::default()
    };

    AppState {
        config: RwLock::new(config),
        monitor_generation: AtomicU64::new(0),
        processed_state: Mutex::new(ProcessedState::default()),
        undo_text: Mutex::new(None),
        history_store: Mutex::new(EncryptedHistoryStore::new()),
        proxy: event_loop.create_proxy(),
        persist_config: false,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MonitorMode;
    use std::sync::atomic::Ordering;

    /// `with_config` / `with_processed_state` / `monitor_generation` の基本動作
    #[test]
    fn test_app_state_helpers() {
        let state = test_app_state();

        assert_eq!(state.with_config(|c| c.mode), RefineMode::Trim);
        state.with_config_mut(|c| c.mode = RefineMode::UrlEncode);
        assert_eq!(state.with_config(|c| c.mode), RefineMode::UrlEncode);

        let ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("hello"),
            ..Default::default()
        };
        state.with_processed_state(|s| *s = ps);
        assert!(state.with_processed_state(|s| s.matches_last_seen("hello")));

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Polling);

        state.with_config_mut(|c| c.interval_ms = 2000);
        assert_eq!(state.with_config(|c| c.interval_ms), 2000);

        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 一時停止フラグの更新
    #[test]
    fn test_paused_accessor() {
        let state = test_app_state();
        assert!(!state.with_config(|c| c.is_paused));
        state.with_config_mut(|c| c.is_paused = true);
        assert!(state.with_config(|c| c.is_paused));
        state.with_config_mut(|c| c.is_paused = false);
        assert!(!state.with_config(|c| c.is_paused));
    }

    /// 履歴機能の更新
    #[test]
    fn test_history_enabled_accessor() {
        let state = test_app_state();
        assert!(!state.with_config(|c| c.history_enabled));
        state.with_config_mut(|c| c.history_enabled = true);
        assert!(state.with_config(|c| c.history_enabled));
    }

    /// 通知設定の更新
    #[test]
    fn test_notification_settings_accessor() {
        let state = test_app_state();

        assert!(!state.with_config(|c| c.notification_settings.enabled));
        state.with_config_mut(|c| c.notification_settings.enabled = true);
        assert!(state.with_config(|c| c.notification_settings.enabled));

        assert!(state.with_config(|c| c.notification_settings.notify_mode));
        state.with_config_mut(|c| c.notification_settings.notify_mode = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_mode));

        assert!(!state.with_config(|c| c.notification_settings.notify_result));
        state.with_config_mut(|c| c.notification_settings.notify_result = true);
        assert!(state.with_config(|c| c.notification_settings.notify_result));

        assert!(state.with_config(|c| c.notification_settings.notify_pause));
        state.with_config_mut(|c| c.notification_settings.notify_pause = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_pause));
    }

    /// `monitor_snapshot` が設定値を正しく反映すること
    #[test]
    fn test_monitor_snapshot_values() {
        let state = test_app_state();
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

    fn collect_history(state: &AppState) -> Vec<String> {
        (0..state.history_len())
            .filter_map(|i| state.get_history_entry(i).map(|s| s.to_string()))
            .collect()
    }

    /// 履歴追加: 空白は無視、重複は先頭移動、上限超過分は削除、clear で空になる
    #[test]
    fn test_history_add_dedup_limit_and_clear() {
        let state = test_app_state();
        let limit = crate::consts::DEFAULT_HISTORY_LIMIT;

        // 空白は無視
        state.add_to_history("   ");
        assert_eq!(state.history_len(), 0);

        // 重複するエントリは先頭に移動する
        state.add_to_history("first");
        state.add_to_history("second");
        state.add_to_history("first");
        let h = collect_history(&state);
        assert_eq!(h[0], "first");
        assert_eq!(h[1], "second");
        assert_eq!(h.len(), 2);

        // history_limit を超えた分は切り捨てられる
        for i in 0..(limit + 5) {
            state.add_to_history(format!("item-{i}"));
        }
        assert_eq!(state.history_len(), limit);
        assert_eq!(collect_history(&state)[0], format!("item-{}", limit + 4));

        // clear_history で履歴が空になること
        state.clear_history();
        assert_eq!(state.history_len(), 0);
    }

    /// 加工成功時に書き戻し本文と観測済み本文が更新されること
    #[test]
    fn test_record_processing_success() {
        let state = test_app_state();
        state.record_processing_success("processed");

        let ps = state.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("processed"));
        assert!(ps.matches_last_written("processed"));
    }

    /// 観測のみの場合は `last_written` を変更しないこと
    #[test]
    fn test_record_clipboard_observed() {
        let state = test_app_state();
        state.with_processed_state(|ps| {
            ps.last_written = Some(ContentFingerprint::from_text("written"));
            ps.last_seen = ContentFingerprint::from_text("old");
        });

        state.record_clipboard_observed("observed");

        let ps = state.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("observed"));
        assert!(ps.matches_last_written("written"));
    }

    /// 履歴復元など外部設定時は書き戻しフラグをクリアすること
    #[test]
    fn test_record_clipboard_set() {
        let state = test_app_state();
        state.with_processed_state(|ps| {
            ps.last_written = Some(ContentFingerprint::from_text("written"));
            ps.last_seen = ContentFingerprint::from_text("old");
        });

        state.record_clipboard_set("restored");

        let ps = state.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("restored"));
        assert_eq!(ps.last_written, None);
    }

    /// 加工取り消し用テキストの記録と取得
    #[test]
    fn test_undo_source_record_and_take() {
        let state = test_app_state();

        assert!(state.take_undo_source().is_none());

        state.record_undo_source("original");
        assert_eq!(
            state.take_undo_source().as_ref().map(|s| s.as_str()),
            Some("original")
        );
        assert!(state.take_undo_source().is_none());
    }

    /// テスト用 `AppState` は実行中アプリの `config.json` を上書きしないこと
    #[test]
    fn test_app_state_disables_config_persistence() {
        let state = test_app_state();
        assert!(!state.is_config_persistence_enabled());
    }
}
