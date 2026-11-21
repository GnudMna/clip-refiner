use std::sync::Arc;
use std::time::Instant;

use super::features::{
    handle_app_control, handle_clips_event, handle_continuous_record_event, handle_favorites_event,
    handle_history_event, handle_monitor_event, handle_notification_event,
    handle_refine_mode_event,
};
use super::menu::TrayMenu;
use super::selector::{ClipSelectorWindow, QuickSelectorWindow};
use super::state::AppState;
use super::worker::ClipboardWorkerHandle;

use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

mod config_reload;
mod context;
mod loop_event;
mod ocr;
mod user_event;

pub(crate) use super::features::{
    copy_registered_clip, delete_registered_clip, move_favorite_mode, refresh_clips_views,
    toggle_continuous_record, toggle_favorite_mode, update_monitor_mode, update_refine,
};
pub(crate) use config_reload::reload_config_with_notification;
pub(crate) use context::AppEventContext;
pub(crate) use loop_event::handle_loop_event;
pub(crate) use ocr::run_ocr_on_image;

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod menu_event_tests;

/// クイックセレクターのフォーカス喪失時に非表示へ遷移すべきか判定する
///
/// 表示直後のフォーカスロスト (Windows の Alt キーイベント等) は無視する
pub(crate) fn should_hide_selector_on_focus_loss(elapsed_ms: u128) -> bool {
    elapsed_ms > 200
}

/// フォーカス喪失で自動非表示する UI ウィンドウ
pub(crate) trait FocusDismissibleSelector {
    /// ウィンドウを非表示にする
    fn hide(&self);
    /// ウィンドウが表示中かどうか
    fn is_visible(&self) -> bool;
}

impl FocusDismissibleSelector for QuickSelectorWindow {
    fn hide(&self) {
        QuickSelectorWindow::hide(self);
    }

    fn is_visible(&self) -> bool {
        QuickSelectorWindow::is_visible(self)
    }
}

impl FocusDismissibleSelector for ClipSelectorWindow {
    fn hide(&self) {
        ClipSelectorWindow::hide(self);
    }

    fn is_visible(&self) -> bool {
        ClipSelectorWindow::is_visible(self)
    }
}

// ======================================================================
// メニューイベント処理
// ======================================================================
/// システムトレイアイコンのメニューから受信したイベントを処理する
///
/// クリックされたメニュー項目の ID に基づいて、アプリケーション設定の変更、
/// 履歴操作、加工モードの切り替え、またはプログラムの終了などを実行する
///
/// # Arguments
/// * `event` - 受信したメニューイベント
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_worker` - クリップボード・ワーカー
pub fn handle_menu_event(
    event: &MenuEvent,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
    quick_selector: Option<&QuickSelectorWindow>,
    control_flow: &mut ControlFlow,
) {
    if handle_app_control(&event.id, menu, state, control_flow) {
        return;
    }
    if handle_history_event(&event.id, menu, state, clipboard_worker) {
        return;
    }
    if handle_continuous_record_event(&event.id, menu, state, clipboard_worker) {
        return;
    }
    if handle_clips_event(&event.id, menu, state, clipboard_worker) {
        return;
    }
    if handle_notification_event(&event.id, menu, state) {
        return;
    }
    if handle_favorites_event(&event.id, menu, state, quick_selector) {
        return;
    }
    if handle_refine_mode_event(&event.id, menu, state, clipboard_worker, quick_selector) {
        return;
    }
    handle_monitor_event(&event.id, menu, state);
}

/// UIウィンドウ (クイックセレクター / 登録クリップセレクター) に関連するイベントを処理する
///
/// 主にフォーカス喪失時の自動非表示処理などを行う
///
/// # Arguments
/// * `event` - 受信したウィンドウイベント
/// * `selector` - セレクターウィンドウのインスタンス
/// * `last_selector_show` - セレクターが最後に表示された時刻
pub fn handle_window_event<S: FocusDismissibleSelector>(
    event: &WindowEvent,
    selector: &S,
    last_selector_show: &Instant,
) {
    if let WindowEvent::Focused(focused) = event
        && !focused
        && selector.is_visible()
        && should_hide_selector_on_focus_loss(last_selector_show.elapsed().as_millis())
    {
        selector.hide();
    }
}
