//! アプリケーション共有状態とカスタムイベント

mod app_event;
mod app_state;
mod lock_ext;
mod monitor_snapshot;

pub use app_event::AppEvent;
pub use app_state::AppState;
pub(crate) use app_state::test_app_state;
pub use lock_ext::LockExt;
pub use monitor_snapshot::{MonitorSnapshot, ProcessedState};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    use crate::config::MonitorMode;
    use crate::refiner::RefineMode;
    use crate::security::ContentFingerprint;

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
        assert_eq!(snap.pipeline, vec![RefineMode::UrlEncode]);
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

    /// テスト用 `AppState` は実行中アプリの `config.toml` を上書きしないこと
    #[test]
    fn test_app_state_disables_config_persistence() {
        let state = test_app_state();
        assert!(!state.is_config_persistence_enabled());
    }
}
