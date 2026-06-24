use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::menu::TrayMenu;
use super::monitor::bump_monitor_generation;
use super::notifier;
use super::selector::SelectorWindow;
use super::state::{AppEvent, AppState};
use super::worker::ClipboardCommand;
use crate::config::HotkeySettings;
use crate::consts;
use crate::hotkey_binding::resolve_hotkey;
use crate::notification;

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
    /// セレクタ表示・非表示用ホットキー
    selector_hotkey: HotKey,
    /// 通知有効・無効切替用ホットキー
    notification_hotkey: HotKey,
    /// 一時停止・再開用ホットキー
    pause_hotkey: HotKey,
    /// アプリケーション終了用ホットキー
    quit_hotkey: HotKey,
    /// 加工取り消し用ホットキー
    undo_hotkey: HotKey,
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

        let selector_hotkey = resolve_hotkey(
            &hotkeys.selector,
            consts::DEFAULT_HOTKEY_SELECTOR,
            "selector",
        );
        let notification_hotkey = resolve_hotkey(
            &hotkeys.notification,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            "notification",
        );
        let pause_hotkey = resolve_hotkey(&hotkeys.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause");
        let quit_hotkey = resolve_hotkey(&hotkeys.quit, consts::DEFAULT_HOTKEY_QUIT, "quit");
        let undo_hotkey = resolve_hotkey(&hotkeys.undo, consts::DEFAULT_HOTKEY_UNDO, "undo");

        let register = |hotkey| manager.register(hotkey).map_err(|e| anyhow::anyhow!(e));

        register(selector_hotkey)?;
        register(notification_hotkey)?;
        register(pause_hotkey)?;
        register(quit_hotkey)?;
        register(undo_hotkey)?;

        Ok(Self {
            _manager: manager,
            selector_hotkey,
            notification_hotkey,
            pause_hotkey,
            quit_hotkey,
            undo_hotkey,
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
    /// セレクタウィンドウのインスタンス (セレクタ操作時のみ必要)
    pub selector: Option<&'a SelectorWindow>,
    /// イベントループの制御フロー
    pub control_flow: &'a mut ControlFlow,
    /// セレクタが最後に表示された時刻(更新用)
    pub last_selector_show: &'a mut Instant,
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

        if event.id == self.selector_hotkey.id() {
            Self::handle_selector_hotkey(ctx);
        } else if event.id == self.notification_hotkey.id() {
            Self::toggle_notification(ctx);
        } else if event.id == self.pause_hotkey.id() {
            Self::toggle_pause(ctx);
        } else if event.id == self.quit_hotkey.id() {
            *ctx.control_flow = ControlFlow::Exit;
        } else if event.id == self.undo_hotkey.id() {
            let _ = ctx.clipboard_tx.send(ClipboardCommand::Undo);
        }
    }

    /// セレクタ表示ホットキーを処理する
    fn handle_selector_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(selector) = ctx.selector else {
            return;
        };

        if selector.is_visible() {
            selector.hide();
        } else {
            *ctx.last_selector_show = Instant::now();
            selector.show(ctx.state.with_config(|c| c.mode));
        }
    }

    /// 成功通知の有効/無効を切り替える
    fn toggle_notification(ctx: &mut HotkeyEventContext<'_>) {
        let new_val = ctx.state.with_config_mut(|c| {
            c.notification_settings.enabled = !c.notification_settings.enabled;
            c.notification_settings.enabled
        });
        ctx.menu.notification.enabled_item.set_checked(new_val);
        ctx.menu.notification.content_submenu.set_enabled(new_val);
        ctx.state.save_config();
        notification::show_notification(
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
        notifier::show_pause_notification(ctx.state, new_paused, "ショートカット");
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
        last_selector_show: Instant,
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
                last_selector_show: Instant::now(),
                clipboard_tx,
            }
        }

        fn ctx(&mut self) -> HotkeyEventContext<'_> {
            HotkeyEventContext {
                state: &self.state,
                menu: &self.menu,
                selector: None,
                control_flow: &mut self.control_flow,
                last_selector_show: &mut self.last_selector_show,
                clipboard_tx: &self.clipboard_tx,
            }
        }
    }

    /// テスト間でホットキー登録が衝突しないよう F キーを割り当てる
    fn test_hotkeys() -> HotkeySettings {
        HotkeySettings {
            selector: "Alt+Shift+F1".to_string(),
            notification: "Alt+Shift+F2".to_string(),
            pause: "Alt+Shift+F3".to_string(),
            quit: "Alt+Shift+F4".to_string(),
            undo: "Alt+Shift+F5".to_string(),
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
        assert!(ctx.menu.notification.content_submenu.is_enabled());

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
        assert!(ctx.menu.notification.content_submenu.is_enabled());
    }
}
