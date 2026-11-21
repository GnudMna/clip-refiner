use std::sync::Arc;

use super::decision::{should_process_clipboard, should_record_continuous_clipboard};

use crate::config::MonitorMode;
use crate::refiner::{
    ClipboardProcessError, ClipboardProcessOutcome, ImageClipboard, RefineContext, RefineMode,
    TextClipboard, process_clipboard_pipeline_io,
};
use crate::security::is_within_clipboard_limit;
use crate::tray::notify;
use crate::tray::state::{AppState, MonitorSnapshot};
use crate::user_error::UserNotification;

// ======================================================================
// 定数
// ======================================================================
/// ポーリングスリープを分割するチック間隔(ミリ秒)
pub(crate) const POLL_TICK_MS: u64 = 50;

/// イベント監視ループのスリープ間隔(ミリ秒)
pub(crate) const EVENT_POLL_MS: u64 = 100;

/// クリップボード観測の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardObserveResult {
    /// クリップボード本文を読み取れず、変更トークンを進めない
    ReadFailed,
    /// 観測のみ (連続記録・スキップ・変更なし)
    Observed,
    /// 加工によりクリップボードを書き換えた
    ClipboardWritten,
}

// ======================================================================
// クリップボード更新処理
// ======================================================================
/// 監視世代の再同期後に取りこぼした連続記録を回復する
///
/// 世代更新とコピーが同じタイミングで重なった場合、トークンだけ進んで
/// キューへ積まれないことがあるため、未観測の新規コピーのみ積む
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `text` - 再同期時に読み取ったクリップボード本文
/// * `token_changed` - 前回トークンから変化した場合は `true`
/// * `already_seen` - 再同期前に既に観測済みの本文だった場合は `true`
/// * `initial_sync` - 監視開始直後の初回同期の場合は `true`
pub(crate) fn recover_continuous_record_on_reset(
    state: &Arc<AppState>,
    text: &str,
    token_changed: bool,
    already_seen: bool,
    initial_sync: bool,
) {
    if initial_sync
        || !token_changed
        || already_seen
        || !state.with_config(|c| c.continuous_record_enabled)
    {
        return;
    }

    if !is_within_clipboard_limit(text) {
        state.record_clipboard_observed(text);
        return;
    }

    let should_record =
        state.with_processed_state(|ps| should_record_continuous_clipboard(ps, text));
    if !should_record {
        return;
    }

    state.record_clipboard_observed(text);
    state.enqueue_continuous_record(text);
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
/// * `ctx` - 加工コンテキスト (正規表現コンパイルキャッシュを保持)
///
/// # Returns
/// * `ClipboardObserveResult` - 観測結果。`ReadFailed` のとき呼び出し元は変更トークンを進めない
pub(crate) fn handle_clipboard_update<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    snap: &MonitorSnapshot,
    event_driven: bool,
    ctx: &RefineContext,
) -> ClipboardObserveResult {
    if !snap.auto_processing_enabled() {
        return ClipboardObserveResult::Observed;
    }

    let text = match clipboard.get_text() {
        Ok(text) => text,
        Err(_) if snap.produces_image() => String::new(),
        Err(_) => return ClipboardObserveResult::ReadFailed,
    };

    if text.is_empty() {
        if !snap.produces_image() {
            return ClipboardObserveResult::Observed;
        }
    } else {
        let should_process =
            state.with_processed_state(|ps| should_process_clipboard(ps, &text, event_driven));

        if !should_process {
            return ClipboardObserveResult::Observed;
        }

        if !is_within_clipboard_limit(&text) {
            crate::log_warn!("クリップボードのテキストが上限を超えているため加工をスキップ");
            state.record_clipboard_observed(&text);
            return ClipboardObserveResult::Observed;
        }
    }

    let outcome = process_clipboard_pipeline_io(clipboard, &snap.pipeline, ctx);
    let updated = record_clipboard_outcome(state, snap, &outcome, &text);

    match &outcome {
        Ok(ClipboardProcessOutcome::Processed(processed)) => {
            notify::show_process_notification(state, &snap.pipeline, processed.as_str());
        }
        Ok(ClipboardProcessOutcome::ImageProcessed { width, height }) => {
            notify::show_image_process_notification(
                state,
                snap.pipeline
                    .last()
                    .copied()
                    .unwrap_or(RefineMode::ExcelToImage),
                *width,
                *height,
            );
        }
        Ok(ClipboardProcessOutcome::Unchanged) | Err(ClipboardProcessError::NoText) => {}
        Err(ClipboardProcessError::NoImage) => {
            // Excel 描画ビットマップの到着待ち。次回のクリップボード更新で再試行する
            crate::log_debug!("Excel 描画画像を待機中");
        }
        Err(ClipboardProcessError::TextTooLarge) => {
            crate::log_warn!("クリップボードのテキストが上限を超えているため加工をスキップ");
        }
        Err(e) => {
            UserNotification::from(e).notify_with_debug(e);
        }
    }

    if updated {
        ClipboardObserveResult::ClipboardWritten
    } else {
        ClipboardObserveResult::Observed
    }
}

/// 連続記録モードでクリップボードの内容更新を検知し、元テキストをキューへ積む
///
/// 加工は行わず、クリップボードへの書き戻しや通知も発生しない
///
/// # Returns
/// * `ClipboardObserveResult` - 読み取り失敗時は `ReadFailed`、それ以外は `Observed`
pub(crate) fn observe_continuous_record<C: TextClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
) -> ClipboardObserveResult {
    let Ok(text) = clipboard.get_text() else {
        return ClipboardObserveResult::ReadFailed;
    };

    let should_record =
        state.with_processed_state(|ps| should_record_continuous_clipboard(ps, &text));

    if !should_record {
        return ClipboardObserveResult::Observed;
    }

    if !is_within_clipboard_limit(&text) {
        crate::log_warn!("クリップボードのテキストが上限を超えているため記録をスキップ");
        state.record_clipboard_observed(&text);
        return ClipboardObserveResult::Observed;
    }

    state.record_clipboard_observed(&text);
    state.enqueue_continuous_record(&text);
    ClipboardObserveResult::Observed
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
        Ok(ClipboardProcessOutcome::ImageProcessed { .. }) => {
            state.record_undo_source(observed_text);
            state.record_image_processing_success(observed_text);
            true
        }
        Ok(ClipboardProcessOutcome::Unchanged)
        | Err(
            ClipboardProcessError::ReadFailed(_)
            | ClipboardProcessError::WriteFailed(_)
            | ClipboardProcessError::NoImage,
        ) => {
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
// トレイメニュー更新
// ======================================================================
/// 監視方式の切り替えに伴い、関連するUIコンポーネントの状態を更新する
///
/// 例えば、イベントモード時は「監視周期」の設定メニューを無効化する
///
/// # Arguments
/// * `menu` - トレイメニュー構造体
/// * `monitor_mode` - 新しく選択された監視方式
pub fn update_monitor_mode_impl(menu: &crate::tray::menu::TrayMenu, monitor_mode: MonitorMode) {
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
    use std::sync::Arc;

    use super::*;

    use crate::config::RegexSettings;
    use crate::refiner::RefineMode;
    use crate::test_helpers::InMemoryTextClipboard;

    fn collect_history(state: &Arc<AppState>) -> Vec<String> {
        (0..state.history_len())
            .filter_map(|i| state.get_history_entry(i).map(|s| s.to_string()))
            .collect()
    }

    fn collect_continuous_record(state: &Arc<AppState>) -> Vec<String> {
        (0..state.continuous_record_len())
            .filter_map(|i| state.get_continuous_record_entry(i).map(|s| s.to_string()))
            .collect()
    }

    fn test_snapshot(
        pipeline: Vec<RefineMode>,
        history_enabled: bool,
        continuous_record_enabled: bool,
    ) -> MonitorSnapshot {
        MonitorSnapshot {
            pipeline,
            interval_ms: 1000,
            is_paused: false,
            history_enabled,
            continuous_record_enabled,
            regex_settings: RegexSettings::default(),
        }
    }

    /// 加工成功時に `processed_state` と履歴が更新されるようにすること
    #[test]
    fn record_outcome_processed_updates_state_and_history() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(vec![RefineMode::Trim], true, false);
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
        let snap = test_snapshot(vec![RefineMode::Trim], true, false);
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

    /// 履歴無効時は履歴に追加しないであるようにすること
    #[test]
    fn record_outcome_skips_history_when_disabled() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(vec![RefineMode::Trim], false, false);
        let outcome = Ok(ClipboardProcessOutcome::Processed("x".to_string()));

        record_clipboard_outcome(&state, &snap, &outcome, "x");
        assert_eq!(state.history_len(), 0);
    }

    /// `NoText` エラー時は観測のみ記録し履歴に追加しないであるようにすること
    #[test]
    fn record_outcome_no_text_observes_only() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let snap = test_snapshot(vec![RefineMode::Trim], true, false);
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
        let snap = test_snapshot(vec![RefineMode::Trim], true, false);
        let outcome = Err(ClipboardProcessError::ReadFailed("detail".to_string()));

        assert!(!record_clipboard_outcome(&state, &snap, &outcome, "source"));

        assert_eq!(collect_history(&state), vec!["source".to_string()]);
    }

    /// 連続記録モードでは元テキストを加工せず履歴へ記録すること
    #[test]
    fn continuous_record_stores_observed_text_without_processing() {
        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        let mut clipboard = InMemoryTextClipboard::with_text("  raw  ");

        assert_eq!(
            observe_continuous_record(&mut clipboard, &state),
            ClipboardObserveResult::Observed
        );
        assert_eq!(clipboard.text(), "  raw  ");
        assert_eq!(
            collect_continuous_record(&state),
            vec!["  raw  ".to_string()]
        );
        assert_eq!(collect_history(&state), Vec::<String>::new());
    }

    /// 監視再同期時は既観測テキストを連続記録キューへ積まないこと
    #[test]
    fn recover_continuous_record_skips_already_seen_text() {
        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        state.record_clipboard_observed("existing");

        recover_continuous_record_on_reset(&state, "existing", true, true, false);

        assert_eq!(collect_continuous_record(&state), Vec::<String>::new());
    }

    /// 監視開始直後の初回同期では既存クリップボードを記録しないこと
    #[test]
    fn recover_continuous_record_skips_initial_sync() {
        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.continuous_record_enabled = true);

        recover_continuous_record_on_reset(&state, "startup", true, false, true);

        assert_eq!(collect_continuous_record(&state), Vec::<String>::new());
    }

    /// 監視再同期と同時の未観測コピーは連続記録キューへ積むこと
    #[test]
    fn recover_continuous_record_enqueues_unseen_copy_on_reset() {
        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.continuous_record_enabled = true);

        recover_continuous_record_on_reset(&state, "fresh-copy", true, false, false);

        assert_eq!(
            collect_continuous_record(&state),
            vec!["fresh-copy".to_string()]
        );
    }
}
