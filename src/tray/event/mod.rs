use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::menu::TrayMenu;
use super::quick_selector::QuickSelectorWindow;
use super::state::AppState;
use super::text_selector::TextSelectorWindow;
use super::worker::ClipboardCommand;

use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

mod app_control;
mod config_reload;
mod history;
mod monitor;
mod notification;
mod refine;
mod texts;

pub(crate) use config_reload::reload_config_with_notification;
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

impl FocusDismissibleSelector for super::quick_selector::QuickSelectorWindow {
    fn hide(&self) {
        QuickSelectorWindow::hide(self);
    }

    fn is_visible(&self) -> bool {
        QuickSelectorWindow::is_visible(self)
    }
}

impl FocusDismissibleSelector for super::text_selector::TextSelectorWindow {
    fn hide(&self) {
        TextSelectorWindow::hide(self);
    }

    fn is_visible(&self) -> bool {
        TextSelectorWindow::is_visible(self)
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
/// * `control_flow` - イベントループの制御フロー
pub fn handle_menu_event(
    event: &MenuEvent,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    control_flow: &mut ControlFlow,
) {
    if app_control::handle_app_control(&event.id, menu, state, control_flow) {
        return;
    }
    if history::handle_history_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    if texts::handle_texts_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    if notification::handle_notification_event(&event.id, menu, state) {
        return;
    }
    if refine::handle_refine_mode_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    monitor::handle_monitor_event(&event.id, menu, state);
}

/// 登録文字列をクリップボードへコピーする
pub(crate) fn copy_registered_text(
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    index: usize,
) {
    texts::copy_registered_text(state, clipboard_tx, index);
}

/// 登録文字列を削除し、メニューとセレクターを更新する
pub(crate) fn delete_registered_text(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    text_selector: &TextSelectorWindow,
    index: usize,
) {
    texts::delete_registered_text(state, menu, text_selector, index);
}

/// 登録文字列メニューとセレクター表示を設定内容に合わせて更新する
pub(crate) fn refresh_texts_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    text_selector: &TextSelectorWindow,
) {
    texts::refresh_texts_views(state, menu, text_selector);
}

/// UIウィンドウ (クイックセレクター / テキストセレクター) に関連するイベントを処理する
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    use crate::config::MonitorMode;
    use crate::refiner::RefineMode;
    use crate::tray::menu::TrayMenu;
    use crate::tray::state::{LockExt, test_app_state};
    use crate::tray::worker::ClipboardCommand;

    /// 表示直後 200ms 以内はフォーカス喪失を無視すること
    #[test]
    fn should_not_hide_selector_immediately_after_show() {
        assert!(!should_hide_selector_on_focus_loss(100));
        assert!(!should_hide_selector_on_focus_loss(200));
    }

    /// 200ms 超過後はフォーカス喪失で非表示にすること
    #[test]
    fn should_hide_selector_after_focus_loss_delay() {
        assert!(should_hide_selector_on_focus_loss(201));
    }

    /// `update_refine` が設定とワーカーコマンドを更新すること
    #[test]
    fn update_refine_updates_config_and_sends_command() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, rx) = mpsc::channel();

        update_refine(&state, &menu, &tx, RefineMode::JsonFormat);

        assert_eq!(state.with_config(|c| c.mode), RefineMode::JsonFormat);
        assert!(
            menu.refine
                .all_items()
                .any(|(item, mode)| *mode == RefineMode::JsonFormat && item.is_checked())
        );
        match rx.recv().expect("ワーカーコマンドが送信される") {
            ClipboardCommand::ProcessMode(mode) => assert_eq!(mode, RefineMode::JsonFormat),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// `update_monitor_mode` が設定と監視周期メニューを更新すること
    #[test]
    fn update_monitor_mode_switches_to_event_and_disables_interval() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");

        monitor::update_monitor_mode(&state, &menu, MonitorMode::Event);

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Event);
        assert!(
            menu.monitor
                .items
                .iter()
                .any(|(item, mode)| *mode == MonitorMode::Event && item.is_checked())
        );
        assert!(!menu.interval.main_submenu.is_enabled());
    }

    /// 同一モードへの `update_monitor_mode` は no-op であること
    #[test]
    fn update_monitor_mode_noop_when_unchanged() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Polling);

        monitor::update_monitor_mode(&state, &menu, MonitorMode::Polling);

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Polling);
        assert!(menu.interval.main_submenu.is_enabled());
    }

    fn menu_event(id: &tray_icon::menu::MenuId) -> MenuEvent {
        MenuEvent { id: id.clone() }
    }

    /// 終了メニューで `ControlFlow::Exit` になること
    #[test]
    fn handle_menu_event_quit_exits() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(menu.quit_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert!(matches!(control_flow, ControlFlow::Exit));
    }

    /// 一時停止チェック ON で設定が一時停止になること
    #[test]
    fn handle_menu_event_pause_enables_paused() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        menu.pause_item.set_checked(true);
        handle_menu_event(
            &menu_event(menu.pause_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert!(state.with_config(|c| c.is_paused));
    }

    /// 履歴クリアで履歴が空になること
    #[test]
    fn handle_menu_event_history_clear() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.history_enabled = true);
        state.add_to_history("entry");
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        menu.refresh_history(&state)
            .expect("履歴メニューの更新に失敗");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(menu.history.clear_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert_eq!(state.history_len(), 0);
    }

    /// 登録文字列の「クリップボードを登録」でワーカーコマンドが送信されること
    #[test]
    fn handle_menu_event_texts_register_sends_command() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, rx) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(menu.texts.register_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert!(matches!(
            rx.recv().expect("ワーカーコマンドが送信される"),
            ClipboardCommand::RegisterFromClipboard
        ));
    }

    /// 履歴項目選択でクリップボードへテキスト送信すること
    #[test]
    fn handle_menu_event_history_select_sends_text() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.history_enabled = true);
        state.add_to_history("copied text");
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        menu.refresh_history(&state)
            .expect("履歴メニューの更新に失敗");
        let record_id = {
            let records = menu.history.records.lock_ignore_poison();
            records[0].0.clone()
        };
        let (tx, rx) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(&record_id),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        match rx.recv().expect("ワーカーコマンドが送信される") {
            ClipboardCommand::SetText(text) => assert_eq!(text.as_str(), "copied text"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// 加工モード選択で設定とワーカーコマンドが更新されること
    #[test]
    fn handle_menu_event_refine_mode_change() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (item, mode) = menu
            .refine
            .all_items()
            .find(|(_, m)| *m == RefineMode::JsonFormat)
            .expect("JsonFormat メニュー項目が存在する");
        let (tx, rx) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert_eq!(state.with_config(|c| c.mode), RefineMode::JsonFormat);
        assert!(item.is_checked());
        match rx.recv().expect("ワーカーコマンドが送信される") {
            ClipboardCommand::ProcessMode(received) => assert_eq!(received, *mode),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// 通知 ON で設定が更新されること
    #[test]
    fn handle_menu_event_notification_enabled() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        menu.notification.enabled_item.set_checked(true);
        handle_menu_event(
            &menu_event(menu.notification.enabled_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert!(state.with_config(|c| c.notification_settings.enabled));
    }

    /// クリップボード内容表示の切替で `notify_result` が更新されること
    #[test]
    fn handle_menu_event_notification_clipboard_content_toggle() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        menu.notification.notify_result_item.set_checked(true);
        handle_menu_event(
            &menu_event(menu.notification.notify_result_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert!(state.with_config(|c| c.notification_settings.notify_result));
    }

    /// 監視周期選択で `interval_ms` が更新されること
    #[test]
    fn handle_menu_event_interval_change() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (item, _) = menu
            .interval
            .items
            .iter()
            .find(|(_, ms)| *ms == 500)
            .expect("0.5秒の監視周期項目が存在する");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert_eq!(state.with_config(|c| c.interval_ms), 500);
        assert!(item.is_checked());
    }

    /// 監視方式メニューで Event モードへ切り替わること
    #[test]
    fn handle_menu_event_monitor_mode_change() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (item, _) = menu
            .monitor
            .items
            .iter()
            .find(|(_, mode)| *mode == MonitorMode::Event)
            .expect("イベント監視項目が存在する");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        handle_menu_event(
            &menu_event(item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Event);
        assert!(!menu.interval.main_submenu.is_enabled());
    }

    /// 履歴有効化で設定が更新されること
    #[test]
    fn handle_menu_event_history_enabled() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let mut control_flow = ControlFlow::Wait;

        menu.history.enabled_item.set_checked(true);
        handle_menu_event(
            &menu_event(menu.history.enabled_item.id()),
            &menu,
            &state,
            &tx,
            &mut control_flow,
        );

        assert!(state.with_config(|c| c.history_enabled));
    }
}
