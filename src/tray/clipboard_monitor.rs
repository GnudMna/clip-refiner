use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::notify;
use super::state::{AppState, MonitorSnapshot, ProcessedState};
use crate::config::MonitorMode;
use crate::platform;
use crate::refiner::{
    ClipboardProcessError, ClipboardProcessOutcome, RefineContext, TextClipboard,
    process_text_clipboard,
};
use crate::security::{ContentFingerprint, is_within_clipboard_limit};

// ======================================================================
// 監視スレッド管理
// ======================================================================
/// クリップボード監視の世代カウンタを進め、ワーカーに監視状態のリセットを通知する
///
/// 監視処理自体はクリップボードワーカースレッド内で実行される
/// 一時停止中 (`is_paused == true`) の場合は何もしない
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn bump_monitor_generation(state: &Arc<AppState>) {
    if state.with_config(|c| c.is_paused) {
        return;
    }

    state.monitor_generation.fetch_add(1, Ordering::SeqCst);
}

// ======================================================================
// クリップボード更新処理
// ======================================================================
/// ポーリングスリープを分割するチック間隔(ミリ秒)
pub(crate) const POLL_TICK_MS: u64 = 50;

/// イベント監視ループのスリープ間隔(ミリ秒)
pub(crate) const EVENT_POLL_MS: u64 = 100;

/// 加工を試みるべきか判定する。スキップする場合は `processed_state` を更新する。
///
/// # Arguments
/// * `ps` - 前回の加工状態
/// * `text` - 現在のクリップボードテキスト
/// * `event_driven` - イベント駆動監視の場合は `true`、ポーリングの場合は `false`
///
/// # Returns
/// * `true` - 加工を試みる
/// * `false` - スキップする
pub(crate) fn should_process_clipboard(
    ps: &mut ProcessedState,
    text: &str,
    event_driven: bool,
) -> bool {
    if text.is_empty() {
        return false;
    }

    // 自身の書き戻しによるクリップボード変更イベントを1回無視
    if ps.matches_last_written(text) {
        ps.last_written = None;
        ps.last_seen = ContentFingerprint::from_text(text);
        return false;
    }

    // ポーリング: 前回と同じ内容なら加工しない
    if !event_driven && ps.matches_last_seen(text) {
        return false;
    }

    true
}

/// クリップボードの内容更新を検知し、必要に応じて加工処理を行う
///
/// 内容に変更があった場合、現在の加工モードを適用し、結果をクリップボードに書き戻す
/// 通知の表示や履歴への追加もここで行われる
///
/// # Arguments
/// * `clipboard` - クリップボード操作用のインスタンス
/// * `state` - アプリケーションの共有状態
/// * `snap` - ループ先頭で取得済みの設定スナップショット
/// * `event_driven` - イベント駆動監視の場合は `true`、ポーリングの場合は `false`
///
/// # Returns
/// * `bool` - 加工が実行され、クリップボードが更新された場合は `true`、それ以外は `false` を返す
pub(crate) fn handle_clipboard_update<C: TextClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    snap: &MonitorSnapshot,
    event_driven: bool,
) -> bool {
    let Ok(text) = clipboard.get_text() else {
        return false;
    };

    let should_process =
        state.with_processed_state(|ps| should_process_clipboard(ps, &text, event_driven));

    if !should_process {
        return false;
    }

    if !is_within_clipboard_limit(&text) {
        crate::log_warn!("クリップボードのテキストが上限を超えているため加工をスキップ");
        state.record_clipboard_observed(&text);
        return false;
    }

    let ctx = RefineContext {
        regex: snap.regex_settings.clone(),
    };
    let outcome = process_text_clipboard(clipboard, snap.mode, &ctx);
    let updated = record_clipboard_outcome(state, snap, &outcome, &text);

    match &outcome {
        Ok(ClipboardProcessOutcome::Processed(processed)) => {
            notify::show_process_notification(state, snap.mode, processed.as_str());
        }
        Ok(ClipboardProcessOutcome::Unchanged) | Err(ClipboardProcessError::NoText) => {}
        Err(ClipboardProcessError::TextTooLarge) => {
            crate::log_warn!("クリップボードのテキストが上限を超えているため加工をスキップ");
        }
        Err(e) => {
            crate::log_error!("クリップボード加工エラー: {} ({:?})", e.user_message(), e);
            platform::show_notification("加工エラー", e.user_message());
        }
    }

    updated
}

/// 加工結果に応じて共有状態と履歴を更新する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `snap` - 監視設定スナップショット
/// * `outcome` - `process_clipboard` の結果
/// * `observed_text` - 加工前に観測したクリップボード本文
///
/// # Returns
/// * `bool` - クリップボードが加工更新された場合は `true`
pub(crate) fn record_clipboard_outcome(
    state: &Arc<AppState>,
    snap: &MonitorSnapshot,
    outcome: &Result<ClipboardProcessOutcome, ClipboardProcessError>,
    observed_text: &str,
) -> bool {
    match outcome {
        Ok(ClipboardProcessOutcome::Processed(processed)) => {
            state.record_undo_source(observed_text);
            state.record_processing_success(processed);
            if snap.history_enabled {
                state.add_to_history(processed.clone());
            }
            true
        }
        Ok(ClipboardProcessOutcome::Unchanged)
        | Err(ClipboardProcessError::ReadFailed(_) | ClipboardProcessError::WriteFailed(_)) => {
            state.record_clipboard_observed(observed_text);
            if snap.history_enabled {
                state.add_to_history(observed_text);
            }
            false
        }
        Err(ClipboardProcessError::NoText | ClipboardProcessError::TextTooLarge) => {
            state.record_clipboard_observed(observed_text);
            false
        }
    }
}

// ======================================================================
// UI更新
// ======================================================================
/// 監視方式の切り替えに伴い、関連するUIコンポーネントの状態を更新する
///
/// 例えば、イベントモード時は「監視周期」の設定メニューを無効化する
///
/// # Arguments
/// * `menu` - トレイメニュー構造体
/// * `monitor_mode` - 新しく選択された監視方式
pub fn update_monitor_mode_impl(menu: &super::menu::TrayMenu, monitor_mode: MonitorMode) {
    match monitor_mode {
        MonitorMode::Event => menu.interval.main_submenu.set_enabled(false),
        MonitorMode::Polling => menu.interval.main_submenu.set_enabled(true),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RegexSettings;
    use crate::refiner::RefineMode;
    use crate::security::ContentFingerprint;

    /// 空文字列は加工対象外とすること
    #[test]
    fn empty_text_is_not_processed() {
        let mut ps = ProcessedState::default();
        assert!(!should_process_clipboard(&mut ps, "", false));
        assert!(!should_process_clipboard(&mut ps, "", true));
    }

    fn collect_history(state: &std::sync::Arc<crate::tray::state::AppState>) -> Vec<String> {
        (0..state.history_len())
            .filter_map(|i| state.get_history_entry(i).map(|s| s.to_string()))
            .collect()
    }

    /// ポーリング時は新しいテキストを加工対象とすること
    #[test]
    fn polling_processes_new_text() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("old"),
            last_written: None,
        };

        assert!(should_process_clipboard(&mut ps, "new", false));
    }

    /// 自身の書き戻しによる変更イベントを1回スキップすること
    #[test]
    fn should_skip_own_write_back_echo() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("input"),
            last_written: Some(ContentFingerprint::from_text("output")),
        };

        assert!(!should_process_clipboard(&mut ps, "output", true));
        assert_eq!(ps.last_written, None);
        assert!(ps.matches_last_seen("output"));
    }

    /// ポーリング時は同一テキストをスキップし、イベント時は再処理すること
    #[test]
    fn polling_skips_unchanged_text() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("same"),
            last_written: None,
        };

        assert!(!should_process_clipboard(&mut ps, "same", false));
        assert!(should_process_clipboard(&mut ps, "same", true));
    }

    /// イベント駆動時は加工済みテキストの再コピーも処理対象とすること
    #[test]
    fn event_mode_allows_recopy_of_processed_output() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("processed"),
            last_written: None,
        };

        // 加工結果と同じ文字列の再コピー(イベント駆動)も加工対象とする
        assert!(should_process_clipboard(&mut ps, "processed", true));
    }

    /// イベント駆動時は元テキストの再コピーも処理対象とすること
    #[test]
    fn event_mode_allows_recopy_of_source_text() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("processed"),
            last_written: None,
        };

        assert!(should_process_clipboard(&mut ps, "  source  ", true));
    }

    /// 一時停止中は `bump_monitor_generation` が世代を進めないこと
    #[test]
    fn bump_monitor_generation_skips_when_paused() {
        use std::sync::atomic::Ordering;

        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.is_paused = true);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 監視中は `bump_monitor_generation` が世代カウンタをインクリメントすること
    #[test]
    fn bump_monitor_generation_increments_when_active() {
        use std::sync::atomic::Ordering;

        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.is_paused = false);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 1);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 2);
    }

    fn test_snapshot(mode: RefineMode, history_enabled: bool) -> MonitorSnapshot {
        MonitorSnapshot {
            mode,
            interval_ms: 1000,
            is_paused: false,
            history_enabled,
            regex_settings: RegexSettings::default(),
        }
    }

    /// 加工成功時に `processed_state` と履歴が更新されること
    #[test]
    fn record_outcome_processed_updates_state_and_history() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(RefineMode::Trim, true);
        let outcome = Ok(ClipboardProcessOutcome::Processed("trimmed".to_string()));

        assert!(record_clipboard_outcome(
            &state,
            &snap,
            &outcome,
            "  trimmed  "
        ));

        let ps = state.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("trimmed"));
        assert!(ps.matches_last_written("trimmed"));
        assert_eq!(collect_history(&state), vec!["trimmed".to_string()]);
        assert_eq!(
            state.take_undo_source().as_ref().map(|s| s.as_str()),
            Some("  trimmed  ")
        );
    }

    /// 変更なし時は観測のみ記録し、履歴に元テキストを追加すること
    #[test]
    fn record_outcome_unchanged_observes_and_adds_history() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(RefineMode::Trim, true);
        let outcome = Ok(ClipboardProcessOutcome::Unchanged);

        assert!(!record_clipboard_outcome(
            &state,
            &snap,
            &outcome,
            "unchanged"
        ));

        let ps = state.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("unchanged"));
        assert!(!ps.matches_last_written("unchanged"));
        assert_eq!(collect_history(&state), vec!["unchanged".to_string()]);
    }

    /// 履歴無効時は履歴に追加しないこと
    #[test]
    fn record_outcome_skips_history_when_disabled() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(RefineMode::Trim, false);
        let outcome = Ok(ClipboardProcessOutcome::Processed("x".to_string()));

        record_clipboard_outcome(&state, &snap, &outcome, "x");
        assert_eq!(state.history_len(), 0);
    }

    /// `NoText` エラー時は観測のみ記録し履歴に追加しないこと
    #[test]
    fn record_outcome_no_text_observes_only() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(RefineMode::Trim, true);
        let outcome = Err(ClipboardProcessError::NoText);

        assert!(!record_clipboard_outcome(&state, &snap, &outcome, ""));

        let ps = state.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen(""));
        assert_eq!(state.history_len(), 0);
    }

    /// 読み取り失敗時は観測を記録し履歴に追加すること
    #[test]
    fn record_outcome_read_error_observes_and_adds_history() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(RefineMode::Trim, true);
        let outcome = Err(ClipboardProcessError::ReadFailed("detail".to_string()));

        assert!(!record_clipboard_outcome(&state, &snap, &outcome, "source"));

        assert_eq!(collect_history(&state), vec!["source".to_string()]);
    }
}
