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

pub use app_event::{
    AppEvent, ClipRef, ClipsEvent, ConfigEvent, FavoritesEvent, HotkeyEvent, MenuRefreshEvent,
    OcrEvent, RecordEvent, SelectorEvent, SettingsEvent, UpdateEvent, WorkerEvent,
};
pub use app_state::AppState;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use app_state::{test_app_state, test_app_state_with_persistence};
pub(crate) use clipboard_session::UndoLookup;
pub use lock_ext::LockExt;
pub use menu_snapshot::MenuSnapshot;
pub use monitor_snapshot::{MonitorSnapshot, ProcessedState};
pub(crate) use settings_pending::{ClipEditRequest, RegexPreviewRequest};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::config::MonitorMode;
    use crate::refiner::RefineMode;

    /// `with_config` / `monitor_generation` が基本どおりに動作すること
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

    /// 履歴機能の有効・無効を更新できるようにすること
    #[test]
    fn test_history_enabled_accessor() {
        let state = test_app_state();
        assert!(!state.with_config(|c| c.history_enabled));
        state.with_config_mut(|c| c.history_enabled = true);
        assert!(state.with_config(|c| c.history_enabled));
    }

    /// `with_config_mut` のクロージャが変更途中でパニックしたら変更前の設定へ戻すこと
    #[test]
    fn with_config_mut_rolls_back_on_panic() {
        let state = test_app_state();
        let before_interval = state.with_config(|c| c.interval_ms);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.with_config_mut(|c| {
                c.mode = RefineMode::UrlEncode;
                c.interval_ms = before_interval + 1;
                panic!("panic while mutating config");
            });
        }));
        assert!(result.is_err());

        // 途中まで書き換えたフィールドも含めて変更前へ戻る
        assert_eq!(state.with_config(|c| c.mode), RefineMode::Trim);
        assert_eq!(state.with_config(|c| c.interval_ms), before_interval);

        // ポイズン解除後も通常どおり変更できる
        state.with_config_mut(|c| c.mode = RefineMode::UrlEncode);
        assert_eq!(state.with_config(|c| c.mode), RefineMode::UrlEncode);
    }

    /// 通知設定を更新できるようにすること
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

        assert!(state.with_config(|c| c.notification_settings.notify_auto_refine));
        state.with_config_mut(|c| c.notification_settings.notify_auto_refine = false);
        assert!(!state.with_config(|c| c.notification_settings.notify_auto_refine));
    }

    /// `monitor_snapshot` が設定値を正しく反映すること
    #[test]
    fn test_monitor_snapshot_values() {
        let state = test_app_state();
        state.with_config_mut(|c| {
            c.mode = RefineMode::UrlEncode;
            c.interval_ms = 1500;
            c.auto_refine_enabled = true;
            c.history_enabled = true;
        });

        let snap = state.monitor_snapshot();
        assert_eq!(snap.pipeline, vec![RefineMode::UrlEncode]);
        assert_eq!(snap.interval_ms, 1500);
        assert!(snap.auto_refine_enabled);
        assert!(snap.history_enabled);
    }

    /// `menu_snapshot` がメニュー用フィールドを正しく反映すること
    #[test]
    fn test_menu_snapshot_values() {
        use crate::config::HotkeySettings;

        let state = test_app_state();
        state.with_config_mut(|c| {
            c.mode = RefineMode::UrlEncode;
            c.pipeline = vec![RefineMode::Trim, RefineMode::UrlEncode];
            c.favorite_modes = vec![RefineMode::Trim];
            c.hotkeys = HotkeySettings {
                auto_refine: "Ctrl+Alt+P".to_string(),
                ..HotkeySettings::default()
            };
            c.history_enabled = true;
            c.continuous_record_enabled = true;
        });

        let snap: MenuSnapshot = state.menu_snapshot();
        assert_eq!(snap.mode, RefineMode::UrlEncode);
        assert!(snap.active_modes.contains(&RefineMode::Trim));
        assert!(snap.active_modes.contains(&RefineMode::UrlEncode));
        assert_eq!(snap.favorite_modes, vec![RefineMode::Trim]);
        assert_eq!(snap.hotkeys.auto_refine, "Ctrl+Alt+P");
        assert!(snap.history_enabled);
        assert!(snap.continuous_record_enabled);
        assert_eq!(snap.regex_active_preset_id, "default");
        assert!(!snap.regex_presets.is_empty());
    }

    /// `auto_processing_enabled` が自動変換・連続記録の状態を反映すること
    #[test]
    fn test_auto_processing_enabled() {
        let state = test_app_state();

        assert!(state.monitor_snapshot().auto_processing_enabled());

        state.with_config_mut(|c| c.auto_refine_enabled = false);
        assert!(!state.monitor_snapshot().auto_processing_enabled());

        state.with_config_mut(|c| c.auto_refine_enabled = true);

        state.with_config_mut(|c| c.continuous_record_enabled = true);
        assert!(!state.monitor_snapshot().auto_processing_enabled());
    }

    /// 履歴追加: 空白は無視、重複は先頭移動、上限超過分は削除、clear で空になるようにすること
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
        let h = state.clipboard_session().history_texts();
        assert_eq!(h[0], "first");
        assert_eq!(h[1], "second");
        assert_eq!(h.len(), 2);

        // history_limit を超えた分は切り捨てられる
        for i in 0..(limit + 5) {
            state.add_to_history(format!("item-{i}"));
        }
        assert_eq!(session.history_len(), limit);
        assert_eq!(
            state.clipboard_session().history_texts()[0],
            format!("item-{}", limit + 4)
        );

        // clear_history で履歴が空になること
        session.clear_history();
        assert_eq!(session.history_len(), 0);
    }

    /// 連続記録追加: 空白は無視、重複を保持し、上限超過分は先頭から破棄、clear で空になるようにすること
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
        let records = state.clipboard_session().continuous_record_texts();
        assert_eq!(records, vec!["dup", "dup"]);

        for i in 0..(limit + 5) {
            state.enqueue_continuous_record(format!("item-{i}"));
        }
        assert_eq!(session.continuous_record_len(), limit);
        let records = state.clipboard_session().continuous_record_texts();
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

    /// テスト用 `AppState` は実行中アプリの `config.toml` を上書きしないようにすること
    #[test]
    fn test_app_state_disables_config_persistence() {
        let state = test_app_state();
        assert!(!state.config_sync().is_persistence_enabled());
    }

    /// 永続化付きテスト用 `AppState` はディスク保存が有効となるようにすること
    #[test]
    fn test_app_state_with_persistence_enables_config_persistence() {
        let state = test_app_state_with_persistence();
        assert!(state.config_sync().is_persistence_enabled());
    }
}
