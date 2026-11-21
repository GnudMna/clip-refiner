use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::clip_selector::ClipSelectorWindow;
use super::menu::TrayMenu;
use super::quick_selector::QuickSelectorWindow;
use super::state::AppState;
use super::worker::ClipboardCommand;

use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

mod app_control;
mod clips;
mod config_reload;
mod favorites;
mod history;
mod monitor;
mod notification;
mod ocr;
mod refine;

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod menu_event_tests;

pub(crate) use config_reload::reload_config_with_notification;
pub(crate) use favorites::{move_favorite_mode, toggle_favorite_mode};
pub(crate) use ocr::run_ocr_on_image;
pub use refine::update_refine;

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
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
/// * `quick_selector` - 表示中の更新に使うクイックセレクター (未使用時は `None`)
/// * `control_flow` - イベントループの制御フロー
pub fn handle_menu_event(
    event: &MenuEvent,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    quick_selector: Option<&QuickSelectorWindow>,
    control_flow: &mut ControlFlow,
) {
    if app_control::handle_app_control(&event.id, menu, state, control_flow) {
        return;
    }
    if history::handle_history_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    if clips::handle_clips_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    if notification::handle_notification_event(&event.id, menu, state) {
        return;
    }
    if favorites::handle_favorites_event(&event.id, menu, state, quick_selector) {
        return;
    }
    if refine::handle_refine_mode_event(&event.id, menu, state, clipboard_tx, quick_selector) {
        return;
    }
    monitor::handle_monitor_event(&event.id, menu, state);
}

/// 登録クリップをクリップボードへコピーする
pub(crate) fn copy_registered_clip(
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    index: usize,
) {
    clips::copy_registered_clip(state, clipboard_tx, index);
}

/// 登録クリップを削除し、メニューとセレクターを更新する
pub(crate) fn delete_registered_clip(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    index: usize,
) {
    clips::delete_registered_clip(state, menu, clip_selector, index);
}

/// 登録クリップメニューとセレクター表示を設定内容に合わせて更新する
pub(crate) fn refresh_clips_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
) {
    clips::refresh_clips_views(state, menu, clip_selector);
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
