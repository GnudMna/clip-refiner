//! アプリケーション独自のユーザーイベント (`AppEvent`) の振り分け

mod misc;
mod selector;
mod settings;
mod system;

use crate::tray::event::AppEventContext;
use crate::tray::state::AppEvent;

// ======================================================================
// ユーザーイベント処理
// ======================================================================
/// アプリケーション独自のユーザーイベント (`AppEvent`) を処理する
pub(crate) fn handle_user_event(ctx: &mut AppEventContext<'_>, event: AppEvent) {
    match event {
        AppEvent::Selector(e) => selector::handle(ctx, e),
        AppEvent::Clips(e) => selector::handle_clips(ctx, e),
        AppEvent::Favorites(e) => selector::handle_favorites(ctx, e),
        AppEvent::MenuRefresh(e) => system::handle_menu_refresh(ctx, e),
        AppEvent::Config(e) => system::handle_config(ctx, e),
        AppEvent::Worker(e) => system::handle_worker(ctx, e),
        AppEvent::Hotkey(e) => system::handle_hotkey(ctx, e),
        AppEvent::Settings(e) => settings::handle_settings(ctx, e),
        AppEvent::Update(e) => settings::handle_update(ctx, e),
        AppEvent::Ocr(e) => misc::handle_ocr(ctx, e),
        AppEvent::Record(e) => misc::handle_record(ctx, e),
        AppEvent::WebView(e) => misc::handle_webview(ctx, e),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::windows::EventLoopBuilderExtWindows;

    use super::handle_user_event;
    use crate::config::HotkeySettings;
    use crate::test_helpers::{stub_worker, test_app_state_arc, test_tray_menu};
    use crate::tray::event::AppEventContext;
    use crate::tray::hotkey::HotkeyHandler;
    use crate::tray::selector::SelectorUi;
    use crate::tray::settings::SettingsWindow;
    use crate::tray::state::{AppEvent, OcrEvent, SelectorEvent, WebViewEvent};

    /// セレクター / OCR の非表示系と `WebView` の異常終了の `AppEvent` を扱えること
    ///
    /// ホットキー登録は OS グローバルのため、衝突回避で 1 テストにまとめる
    #[test]
    fn hide_and_dispose_overlay_events() {
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        let state = test_app_state_arc();
        let menu = test_tray_menu(&state);
        let (clipboard_worker, _) = stub_worker(&state);
        let selectors = SelectorUi::new(&state.proxy, Arc::clone(&clipboard_worker));
        let settings = SettingsWindow::new_shell(state.proxy.clone(), Arc::clone(&state));

        // 他テスト (Alt+Ctrl+Shift+F* / 英字) と衝突しないキー
        let hotkeys = HotkeySettings {
            quick_selector: "Ctrl+Alt+Shift+1".to_string(),
            notification: "Ctrl+Alt+Shift+2".to_string(),
            auto_refine: "Ctrl+Alt+Shift+3".to_string(),
            quit: "Ctrl+Alt+Shift+4".to_string(),
            undo: "Ctrl+Alt+Shift+5".to_string(),
            clip_selector: "Ctrl+Alt+Shift+6".to_string(),
            ocr: "Ctrl+Alt+Shift+7".to_string(),
            continuous_record: "Ctrl+Alt+Shift+8".to_string(),
            continuous_record_undo: "Ctrl+Alt+Shift+9".to_string(),
            continuous_record_paste: "Ctrl+Alt+Shift+0".to_string(),
            favorite_mode_slots: Vec::new(),
        };
        let mut hotkey_handler = HotkeyHandler::new(&hotkeys, &[])
            .expect("テスト用ホットキーの登録に失敗")
            .0;
        let mut last_quick_selector_show = Instant::now();
        let mut last_clip_selector_show = Instant::now();
        let mut control_flow = ControlFlow::Wait;

        let mut ctx = AppEventContext {
            state: &state,
            menu: &menu,
            selectors: &selectors,
            settings: &settings,
            last_quick_selector_show: &mut last_quick_selector_show,
            last_clip_selector_show: &mut last_clip_selector_show,
            hotkey_handler: &mut hotkey_handler,
            clipboard_worker: &clipboard_worker,
            event_loop: &event_loop,
            control_flow: &mut control_flow,
        };

        selectors.ocr.force_visible_for_test(true);
        assert!(selectors.ocr.is_visible());
        handle_user_event(&mut ctx, OcrEvent::DisposeOverlay.into());
        assert!(!selectors.ocr.is_visible());

        selectors.quick.force_visible_for_test(true);
        selectors.clip.force_visible_for_test(true);
        handle_user_event(&mut ctx, SelectorEvent::HideQuick.into());
        handle_user_event(&mut ctx, SelectorEvent::HideClip.into());
        assert!(!selectors.quick.is_visible());
        assert!(!selectors.clip.is_visible());

        // どのウィンドウにも一致しない異常終了の通知は何も破棄しない
        // SAFETY: テスト用の識別子として比較にのみ使う
        let unknown = unsafe { tao::window::WindowId::dummy() };
        handle_user_event(&mut ctx, WebViewEvent::ProcessFailed(unknown).into());
        assert!(selectors.quick.id().is_none());
        assert!(settings.id().is_none());
    }
}
