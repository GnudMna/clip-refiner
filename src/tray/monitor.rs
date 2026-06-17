use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::notifier;
use super::state::{AppState, MonitorSnapshot, ProcessedState};
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::{ClipboardProcessError, ClipboardProcessOutcome, process_clipboard};

use arboard::Clipboard;

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
pub fn bump_monitor_generation(state: Arc<AppState>) {
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
    if ps.last_written_text.as_deref() == Some(text) {
        ps.last_written_text = None;
        ps.last_seen_text = text.to_string();
        return false;
    }

    // ポーリング: 前回と同じ内容なら加工しない
    if !event_driven && text == ps.last_seen_text {
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
pub(crate) fn handle_clipboard_update(
    clipboard: &mut Clipboard,
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

    match process_clipboard(clipboard, snap.mode) {
        Ok(ClipboardProcessOutcome::Processed(processed)) => {
            state.record_processing_success(&processed);
            notifier::show_process_notification(state, snap.mode, &processed);

            if snap.history_enabled {
                state.add_to_history(processed);
            }
            true
        }
        Ok(ClipboardProcessOutcome::Unchanged) => {
            state.record_clipboard_observed(&text);

            if snap.history_enabled {
                state.add_to_history(text);
            }
            false
        }
        Err(ClipboardProcessError::NoText) => {
            state.record_clipboard_observed(&text);
            false
        }
        Err(e) => {
            crate::log_error!("クリップボード加工エラー: {} ({:?})", e.user_message(), e);
            notification::show_notification("加工エラー", e.user_message());
            state.record_clipboard_observed(&text);

            if snap.history_enabled {
                state.add_to_history(text);
            }
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

    /// 自身の書き戻しによる変更イベントを1回スキップすること
    #[test]
    fn should_skip_own_write_back_echo() {
        let mut ps = ProcessedState {
            last_seen_text: "input".to_string(),
            last_written_text: Some("output".to_string()),
        };

        assert!(!should_process_clipboard(&mut ps, "output", true));
        assert_eq!(ps.last_written_text, None);
        assert_eq!(ps.last_seen_text, "output");
    }

    /// ポーリング時は同一テキストをスキップし、イベント時は再処理すること
    #[test]
    fn polling_skips_unchanged_text() {
        let mut ps = ProcessedState {
            last_seen_text: "same".to_string(),
            last_written_text: None,
        };

        assert!(!should_process_clipboard(&mut ps, "same", false));
        assert!(should_process_clipboard(&mut ps, "same", true));
    }

    /// イベント駆動時は加工済みテキストの再コピーも処理対象とすること
    #[test]
    fn event_mode_allows_recopy_of_processed_output() {
        let mut ps = ProcessedState {
            last_seen_text: "processed".to_string(),
            last_written_text: None,
        };

        // 加工結果と同じ文字列の再コピー(イベント駆動)も加工対象とする
        assert!(should_process_clipboard(&mut ps, "processed", true));
    }

    /// イベント駆動時は元テキストの再コピーも処理対象とすること
    #[test]
    fn event_mode_allows_recopy_of_source_text() {
        let mut ps = ProcessedState {
            last_seen_text: "processed".to_string(),
            last_written_text: None,
        };

        assert!(should_process_clipboard(&mut ps, "  source  ", true));
    }

    /// 一時停止中は bump_monitor_generation が世代を進めないこと
    #[test]
    fn bump_monitor_generation_skips_when_paused() {
        use crate::tray::state::AppEvent;
        use std::sync::{Arc, atomic::Ordering};
        use tao::event_loop::EventLoopBuilder;
        #[cfg(windows)]
        use tao::platform::windows::EventLoopBuilderExtWindows;

        #[cfg(windows)]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        #[cfg(not(windows))]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

        let state = Arc::new(AppState::new(event_loop.create_proxy()));
        state.with_config_mut(|c| c.is_paused = true);

        bump_monitor_generation(Arc::clone(&state));
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 監視中は bump_monitor_generation が世代カウンタをインクリメントすること
    #[test]
    fn bump_monitor_generation_increments_when_active() {
        use crate::tray::state::AppEvent;
        use std::sync::{Arc, atomic::Ordering};
        use tao::event_loop::EventLoopBuilder;
        #[cfg(windows)]
        use tao::platform::windows::EventLoopBuilderExtWindows;

        #[cfg(windows)]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        #[cfg(not(windows))]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

        let state = Arc::new(AppState::new(event_loop.create_proxy()));
        state.with_config_mut(|c| c.is_paused = false);

        bump_monitor_generation(Arc::clone(&state));
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 1);

        bump_monitor_generation(Arc::clone(&state));
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 2);
    }
}
