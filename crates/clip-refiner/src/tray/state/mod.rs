//! アプリケーション共有状態とカスタムイベント

mod app_event;
mod app_state;
mod clipboard_session;
mod config_sync;
mod lock_ext;
mod menu_snapshot;
mod monitor_snapshot;
mod settings_pending;
mod worker_state;

pub use app_event::AppEvent;
pub use app_state::AppState;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use app_state::{test_app_state, test_app_state_with_persistence};
pub use lock_ext::LockExt;
pub use menu_snapshot::MenuSnapshot;
pub use monitor_snapshot::{MonitorSnapshot, ProcessedState};
pub(crate) use settings_pending::RegexPreviewRequest;

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    use crate::config::MonitorMode;
    use crate::refiner::RefineMode;

    /// `with_config` / `monitor_generation` の基本動作できるようにすること
    #[test]
    fn test_app_state_helpers() {
        let state = test_app_state();

        assert_eq!(state.with_config(|c| c.mode), RefineMode::Trim);
        state.with_config_mut(|c| c.mode = RefineMode::UrlEncode);
        assert_eq!(state.with_config(|c| c.mode), RefineMode::UrlEncode);

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Event);

        state.with_config_mut(|c| c.interval_ms = 2000);
        assert_eq!(state.with_config(|c| c.interval_ms), 2000);

        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 一時停止フラグの更新できるようにすること
    #[test]
    fn test_paused_accessor() {
        let state = test_app_state();
        assert!(!state.with_config(|c| c.is_paused));
        state.with_config_mut(|c| c.is_paused = true);
        assert!(state.with_config(|c| c.is_paused));
        state.with_config_mut(|c| c.is_paused = false);
        assert!(!state.with_config(|c| c.is_paused));
    }

    /// 履歴機能の更新できるようにすること
    #[test]
    fn test_history_enabled_accessor() {
        let state = test_app_state();
        assert!(!state.with_config(|c| c.history_enabled));
        state.with_config_mut(|c| c.history_enabled = true);
        assert!(state.with_config(|c| c.history_enabled));
    }

    /// 通知設定の更新できるようにすること
    #[test]
    fn test_notification_settings_accessor() {
        let state = test_app_state();

        assert!(!state.with_config(|c| c.notification_settings.enabled));
        state.with_config_mut(|c| c.notification_settings.enabled = true);
        assert!(state.with_config(|c| c.notification_settings.enabled));

        assert!(state.with_config(|c| c.notification_settings.notify_process));
        state.with_config_mut(|c| c.notification_settings.notify_process = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_process));

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
        assert_eq!(snap.pipeline, vec![RefineMode::UrlEncode]);
        assert_eq!(snap.interval_ms, 1500);
        assert!(snap.is_paused);
        assert!(snap.history_enabled);
    }

    /// `menu_snapshot` がメニュー用フィールドを正しく反映すること
    #[test]
    fn test_menu_snapshot_values() {
        use crate::config::{HotkeySettings, MonitorMode};

        let state = test_app_state();
        state.with_config_mut(|c| {
            c.mode = RefineMode::UrlEncode;
            c.pipeline = vec![RefineMode::Trim, RefineMode::UrlEncode];
            c.favorite_modes = vec![RefineMode::Trim];
            c.hotkeys = HotkeySettings {
                pause: "Ctrl+Alt+P".to_string(),
                ..HotkeySettings::default()
            };
            c.monitor_mode = MonitorMode::Polling;
            c.interval_ms = 2500;
            c.history_enabled = true;
            c.continuous_record_enabled = true;
            c.notification_settings.enabled = true;
            c.notification_settings.notify_result = true;
            c.is_paused = true;
        });

        let snap: MenuSnapshot = state.menu_snapshot();
        assert_eq!(snap.mode, RefineMode::UrlEncode);
        assert!(snap.active_modes.contains(&RefineMode::Trim));
        assert!(snap.active_modes.contains(&RefineMode::UrlEncode));
        assert_eq!(snap.favorite_modes, vec![RefineMode::Trim]);
        assert_eq!(snap.hotkeys.pause, "Ctrl+Alt+P");
        assert_eq!(snap.monitor_mode, MonitorMode::Polling);
        assert_eq!(snap.interval_ms, 2500);
        assert!(snap.history_enabled);
        assert!(snap.continuous_record_enabled);
        assert!(snap.notification_settings.enabled);
        assert!(snap.notification_settings.notify_result);
        assert!(snap.is_paused);
    }

    /// `auto_processing_enabled` が連続記録と一時停止の状態を反映すること
    #[test]
    fn test_auto_processing_enabled() {
        let state = test_app_state();

        let snap = state.monitor_snapshot();
        assert!(snap.auto_processing_enabled());

        state.with_config_mut(|c| c.continuous_record_enabled = true);
        assert!(!state.monitor_snapshot().auto_processing_enabled());

        state.with_config_mut(|c| {
            c.continuous_record_enabled = false;
            c.is_paused = true;
        });
        assert!(!state.monitor_snapshot().auto_processing_enabled());
    }

    fn collect_history(state: &AppState) -> Vec<String> {
        let session = state.clipboard_session();
        (0..session.history_len())
            .filter_map(|i| session.get_history_entry(i).map(|s| s.to_string()))
            .collect()
    }

    /// 履歴追加: 空白は無視、重複は先頭移動、上限超過分は削除、clear で空になること
    ///
    /// `add_to_history` は設定の `history_limit` を参照する横断処理として検証する
    #[test]
    fn test_history_add_dedup_limit_and_clear() {
        let state = test_app_state();
        let limit = crate::consts::DEFAULT_HISTORY_LIMIT;
        let session = state.clipboard_session();

        // 空白は無視
        state.add_to_history("   ");
        assert_eq!(session.history_len(), 0);

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
        assert_eq!(session.history_len(), limit);
        assert_eq!(collect_history(&state)[0], format!("item-{}", limit + 4));

        // clear_history で履歴が空になること
        session.clear_history();
        assert_eq!(session.history_len(), 0);
    }

    fn collect_continuous_record(state: &AppState) -> Vec<String> {
        let session = state.clipboard_session();
        (0..session.continuous_record_len())
            .filter_map(|i| {
                session
                    .get_continuous_record_entry(i)
                    .map(|s| s.to_string())
            })
            .collect()
    }

    /// 連続記録追加: 空白は無視、重複を保持し、上限超過分は先頭から破棄、clear で空になること
    ///
    /// `enqueue_continuous_record` は設定の `continuous_record_limit` を参照する横断処理として検証する
    #[test]
    fn test_continuous_record_enqueue_limit_and_clear() {
        let state = test_app_state();
        let limit = crate::consts::DEFAULT_CONTINUOUS_RECORD_LIMIT;
        let session = state.clipboard_session();

        state.enqueue_continuous_record("   ");
        assert_eq!(session.continuous_record_len(), 0);

        state.enqueue_continuous_record("dup");
        state.enqueue_continuous_record("dup");
        let records = collect_continuous_record(&state);
        assert_eq!(records, vec!["dup", "dup"]);

        for i in 0..(limit + 5) {
            state.enqueue_continuous_record(format!("item-{i}"));
        }
        assert_eq!(session.continuous_record_len(), limit);
        let records = collect_continuous_record(&state);
        assert_eq!(records.first().map(String::as_str), Some("item-5"));

        session.clear_continuous_record();
        assert_eq!(session.continuous_record_len(), 0);
    }

    /// 連続記録キュー先頭取り出しと位置指定排出が FIFO 順で動作すること
    #[test]
    fn test_continuous_record_discharge_fifo() {
        let state = test_app_state();
        state.enqueue_continuous_record("first");
        state.enqueue_continuous_record("second");
        state.enqueue_continuous_record("first");

        assert_eq!(
            state
                .discharge_continuous_record_front()
                .map(|s| s.to_string()),
            Some("first".to_string())
        );
        assert_eq!(
            state
                .discharge_continuous_record_at(0)
                .map(|s| s.to_string()),
            Some("second".to_string())
        );
        assert_eq!(
            state
                .discharge_continuous_record_front()
                .map(|s| s.to_string()),
            Some("first".to_string())
        );
        assert_eq!(state.clipboard_session().continuous_record_len(), 0);
    }

    /// 連続記録の直近 1 件取り消しが末尾から動作すること
    #[test]
    fn test_continuous_record_undo_last() {
        let state = test_app_state();
        assert!(!state.undo_last_continuous_record());

        state.enqueue_continuous_record("first");
        state.enqueue_continuous_record("second");
        assert!(state.undo_last_continuous_record());
        assert_eq!(state.clipboard_session().continuous_record_len(), 1);
        assert_eq!(
            state
                .clipboard_session()
                .get_continuous_record_entry(0)
                .as_ref()
                .map(|s| s.as_str()),
            Some("first")
        );
    }

    /// テスト用 `AppState` は実行中アプリの `config.toml` を上書きしないこと
    #[test]
    fn test_app_state_disables_config_persistence() {
        let state = test_app_state();
        assert!(!state.config_sync().is_persistence_enabled());
    }

    /// 永続化付きテスト用 `AppState` はディスク保存が有効であること
    #[test]
    fn test_app_state_with_persistence_enables_config_persistence() {
        let state = test_app_state_with_persistence();
        assert!(state.config_sync().is_persistence_enabled());
    }
}
