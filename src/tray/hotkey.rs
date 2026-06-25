use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::clipboard_monitor::bump_monitor_generation;
use super::menu::TrayMenu;
use super::notify;
use super::quick_selector::QuickSelectorWindow;
use super::state::{AppEvent, AppState};
use super::text_selector::TextSelectorWindow;
use super::worker::ClipboardCommand;
use crate::config::{AppConfig, HotkeySettings};
use crate::consts;
use crate::hotkey_binding::resolve_hotkey;
use crate::platform;

use anyhow::Result;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::HotKey};
use tao::event_loop::{ControlFlow, EventLoopProxy};

// ======================================================================
// ホットキーハンドラ構造体
// ======================================================================
/// グローバルホットキーの登録と管理を行う構造体
///
/// アプリケーションが非アクティブな状態でも、特定のキー入力を監視して
/// モード選択UIの表示や設定の切り替えなどを実行する
pub struct HotkeyHandler {
    /// ホットキーマネージャーのインスタンス保持用
    _manager: GlobalHotKeyManager,
    /// クイックセレクター表示・非表示用ホットキー
    quick_selector_hotkey: HotKey,
    /// 通知有効・無効切替用ホットキー
    notification_hotkey: HotKey,
    /// 一時停止・再開用ホットキー
    pause_hotkey: HotKey,
    /// アプリケーション終了用ホットキー
    quit_hotkey: HotKey,
    /// 加工取り消し用ホットキー
    undo_hotkey: HotKey,
    /// 登録文字列セレクタ表示・非表示用ホットキー
    text_selector_hotkey: HotKey,
}

// ======================================================================
// 初期化・登録
// ======================================================================
impl HotkeyHandler {
    /// ホットキーハンドラを初期化し、各種ショートカットをシステムに登録する
    ///
    /// # Arguments
    /// * `hotkeys` - 設定ファイルから読み込んだホットキー割り当て
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `HotkeyHandler` インスタンス。登録に失敗した場合はエラーを返す
    pub fn new(hotkeys: &HotkeySettings) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;

        let quick_selector_hotkey = resolve_hotkey(
            &hotkeys.quick_selector,
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            "quick_selector",
        );
        let notification_hotkey = resolve_hotkey(
            &hotkeys.notification,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            "notification",
        );
        let pause_hotkey = resolve_hotkey(&hotkeys.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause");
        let quit_hotkey = resolve_hotkey(&hotkeys.quit, consts::DEFAULT_HOTKEY_QUIT, "quit");
        let undo_hotkey = resolve_hotkey(&hotkeys.undo, consts::DEFAULT_HOTKEY_UNDO, "undo");
        let text_selector_hotkey = resolve_hotkey(
            &hotkeys.text_selector,
            consts::DEFAULT_HOTKEY_TEXT_SELECTOR,
            "text_selector",
        );

        let register = |hotkey| manager.register(hotkey).map_err(|e| anyhow::anyhow!(e));

        register(quick_selector_hotkey)?;
        register(notification_hotkey)?;
        register(pause_hotkey)?;
        register(quit_hotkey)?;
        register(undo_hotkey)?;
        register(text_selector_hotkey)?;

        Ok(Self {
            _manager: manager,
            quick_selector_hotkey,
            notification_hotkey,
            pause_hotkey,
            quit_hotkey,
            undo_hotkey,
            text_selector_hotkey,
        })
    }
}

// ======================================================================
// イベントリスナー
// ======================================================================
impl HotkeyHandler {
    /// ホットキーイベントを受信するためのバックグラウンドスレッドを開始する
    ///
    /// 受信したイベントは `proxy` を介してメインのイベントループへ転送される
    ///
    /// # Arguments
    /// * `proxy` - UIスレッド(イベントループ)へイベントを送信するためのプロキシ
    pub fn start_event_listener(proxy: EventLoopProxy<AppEvent>) {
        std::thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                let _ = proxy.send_event(AppEvent::Hotkey(event));
            }
        });
    }
}

// ======================================================================
// イベント処理コンテキスト
// ======================================================================
/// ホットキーイベント処理に必要な UI 状態の参照
pub struct HotkeyEventContext<'a> {
    /// アプリケーションの共有状態
    pub state: &'a Arc<AppState>,
    /// トレイメニュー構造体
    pub menu: &'a TrayMenu,
    /// クイックセレクターウィンドウのインスタンス (クイックセレクター操作時のみ必要)
    pub quick_selector: Option<&'a QuickSelectorWindow>,
    /// テキストセレクターウィンドウのインスタンス (テキストセレクター操作時のみ必要)
    pub text_selector: Option<&'a TextSelectorWindow>,
    /// イベントループの制御フロー
    pub control_flow: &'a mut ControlFlow,
    /// クイックセレクターが最後に表示された時刻(更新用)
    pub last_quick_selector_show: &'a mut Instant,
    /// テキストセレクターが最後に表示された時刻(更新用)
    pub last_text_selector_show: &'a mut Instant,
    /// クリップボード・ワーカーへの送信チャネル
    pub clipboard_tx: &'a Sender<ClipboardCommand>,
}

// ======================================================================
// イベント処理
// ======================================================================
impl HotkeyHandler {
    /// 受信したホットキーイベントを解析し、対応するアクションを実行する
    ///
    /// # Arguments
    /// * `event` - 受信したホットキーイベント
    /// * `ctx` - イベント処理に必要な UI 状態
    pub fn handle_event(&self, event: GlobalHotKeyEvent, ctx: &mut HotkeyEventContext<'_>) {
        if event.state != global_hotkey::HotKeyState::Pressed {
            return;
        }

        if event.id == self.quick_selector_hotkey.id() {
            Self::handle_quick_selector_hotkey(ctx);
        } else if event.id == self.notification_hotkey.id() {
            Self::toggle_notification(ctx);
        } else if event.id == self.pause_hotkey.id() {
            Self::toggle_pause(ctx);
        } else if event.id == self.quit_hotkey.id() {
            *ctx.control_flow = ControlFlow::Exit;
        } else if event.id == self.undo_hotkey.id() {
            let _ = ctx.clipboard_tx.send(ClipboardCommand::Undo);
        } else if event.id == self.text_selector_hotkey.id() {
            Self::handle_text_selector_hotkey(ctx);
        }
    }

    /// クイックセレクター表示ホットキーを処理する
    fn handle_quick_selector_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(quick_selector) = ctx.quick_selector else {
            return;
        };

        if quick_selector.is_visible() {
            quick_selector.hide();
        } else {
            if let Some(text_selector) = ctx.text_selector
                && text_selector.is_visible()
            {
                text_selector.hide();
            }
            *ctx.last_quick_selector_show = Instant::now();
            quick_selector.show(ctx.state.with_config(|c| c.mode));
        }
    }

    /// テキストセレクター表示ホットキーを処理する
    fn handle_text_selector_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(text_selector) = ctx.text_selector else {
            return;
        };

        if text_selector.is_visible() {
            text_selector.hide();
        } else {
            if let Some(quick_selector) = ctx.quick_selector
                && quick_selector.is_visible()
            {
                quick_selector.hide();
            }
            *ctx.last_text_selector_show = Instant::now();
            let texts_json = ctx.state.with_config(AppConfig::texts_to_json_list);
            text_selector.show(&texts_json);
        }
    }

    /// 成功通知の有効/無効を切り替える
    fn toggle_notification(ctx: &mut HotkeyEventContext<'_>) {
        let new_val = ctx.state.with_config_mut(|c| {
            c.notification_settings.enabled = !c.notification_settings.enabled;
            c.notification_settings.enabled
        });
        ctx.menu.notification.enabled_item.set_checked(new_val);
        ctx.state.save_config();
        platform::show_notification(
            "ショートカット",
            if new_val {
                "成功通知を有効にしました"
            } else {
                "成功通知を無効にしました"
            },
        );
    }

    /// 監視の一時停止/再開を切り替える
    fn toggle_pause(ctx: &mut HotkeyEventContext<'_>) {
        let new_paused = ctx.state.with_config_mut(|c| {
            c.is_paused = !c.is_paused;
            c.is_paused
        });
        ctx.menu.pause_item.set_checked(new_paused);
        ctx.state.save_config();
        notify::show_pause_notification(ctx.state, new_paused, "ショートカット");
        bump_monitor_generation(ctx.state);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
impl HotkeyHandler {
    /// テスト用: 終了ホットキーの ID を返す
    pub(crate) fn quit_hotkey_id(&self) -> u32 {
        self.quit_hotkey.id()
    }

    /// テスト用: 一時停止ホットキーの ID を返す
    pub(crate) fn pause_hotkey_id(&self) -> u32 {
        self.pause_hotkey.id()
    }

    /// テスト用: 通知切替ホットキーの ID を返す
    pub(crate) fn notification_hotkey_id(&self) -> u32 {
        self.notification_hotkey.id()
    }

    /// テスト用: 取り消しホットキーの ID を返す
    pub(crate) fn undo_hotkey_id(&self) -> u32 {
        self.undo_hotkey.id()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Instant;

    use super::*;

    use crate::config::HotkeySettings;
    use crate::tray::menu::TrayMenu;
    use crate::tray::state::test_app_state;
    use crate::tray::worker::ClipboardCommand;

    use global_hotkey::HotKeyState;
    use tao::event_loop::ControlFlow;

    struct HotkeyTestContext {
        state: Arc<AppState>,
        menu: TrayMenu,
        control_flow: ControlFlow,
        last_quick_selector_show: Instant,
        last_text_selector_show: Instant,
        clipboard_tx: Sender<ClipboardCommand>,
    }

    impl HotkeyTestContext {
        fn new() -> Self {
            let state = Arc::new(test_app_state());
            let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
            let (clipboard_tx, _) = mpsc::channel();

            Self {
                state,
                menu,
                control_flow: ControlFlow::Wait,
                last_quick_selector_show: Instant::now(),
                last_text_selector_show: Instant::now(),
                clipboard_tx,
            }
        }

        fn ctx(&mut self) -> HotkeyEventContext<'_> {
            HotkeyEventContext {
                state: &self.state,
                menu: &self.menu,
                quick_selector: None,
                text_selector: None,
                control_flow: &mut self.control_flow,
                last_quick_selector_show: &mut self.last_quick_selector_show,
                last_text_selector_show: &mut self.last_text_selector_show,
                clipboard_tx: &self.clipboard_tx,
            }
        }
    }

    /// テスト間でホットキー登録が衝突しないよう F キーを割り当てる
    fn test_hotkeys() -> HotkeySettings {
        HotkeySettings {
            quick_selector: "Alt+Shift+F1".to_string(),
            notification: "Alt+Shift+F2".to_string(),
            pause: "Alt+Shift+F3".to_string(),
            quit: "Alt+Shift+F4".to_string(),
            undo: "Alt+Shift+F5".to_string(),
            text_selector: "Alt+Shift+F6".to_string(),
        }
    }

    /// ホットキーイベント処理の一連の挙動を検証する
    ///
    /// グローバルホットキーは OS に残るため 1 テストにまとめる
    #[test]
    fn hotkey_event_handling() {
        let handler = HotkeyHandler::new(&test_hotkeys()).expect("テスト用ホットキーの登録に失敗");
        let mut ctx = HotkeyTestContext::new();

        // キー解放は無視
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.quit_hotkey_id(),
                state: HotKeyState::Released,
            },
            &mut ctx.ctx(),
        );
        assert!(matches!(ctx.control_flow, ControlFlow::Wait));

        // 一時停止
        assert!(!ctx.state.with_config(|c| c.is_paused));
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.pause_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(ctx.state.with_config(|c| c.is_paused));
        assert!(ctx.menu.pause_item.is_checked());

        // 通知
        assert!(!ctx.state.with_config(|c| c.notification_settings.enabled));
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.notification_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(ctx.state.with_config(|c| c.notification_settings.enabled));
        assert!(ctx.menu.notification.enabled_item.is_checked());

        // 取り消し
        let (tx, rx) = mpsc::channel();
        ctx.clipboard_tx = tx;
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.undo_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(matches!(
            rx.recv().expect("ワーカーコマンドが送信される"),
            ClipboardCommand::Undo
        ));

        // 終了
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.quit_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(matches!(ctx.control_flow, ControlFlow::Exit));
    }

    /// `toggle_pause` が設定とメニューを更新すること
    #[test]
    fn toggle_pause_updates_state_and_menu() {
        let mut ctx = HotkeyTestContext::new();
        HotkeyHandler::toggle_pause(&mut ctx.ctx());
        assert!(ctx.state.with_config(|c| c.is_paused));
        assert!(ctx.menu.pause_item.is_checked());
    }

    /// `toggle_notification` が設定とメニューを更新すること
    #[test]
    fn toggle_notification_updates_state_and_menu() {
        let mut ctx = HotkeyTestContext::new();
        HotkeyHandler::toggle_notification(&mut ctx.ctx());
        assert!(ctx.state.with_config(|c| c.notification_settings.enabled));
        assert!(ctx.menu.notification.enabled_item.is_checked());
    }
}
