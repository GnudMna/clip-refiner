use std::sync::Arc;
use std::time::Instant;

use super::menu::TrayMenu;
use super::monitor::spawn_monitor_thread;
use super::notifier;
use super::selector::SelectorWindow;
use super::state::{AppEvent, AppState};
use crate::notification;

use anyhow::Result;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use tao::event_loop::{ControlFlow, EventLoopProxy};

// ======================================================================
// ホットキーハンドラ構造体
// ======================================================================
/// グローバルホットキーの登録と管理を行う構造体
///
/// アプリケーションが非アクティブな状態でも、特定のキー入力を監視して
/// モード選択UIの表示や設定の切り替えなどを実行します。
pub struct HotkeyHandler {
    /// ホットキーマネージャーのインスタンス保持用
    _manager: GlobalHotKeyManager,
    /// セレクタ表示・非表示用ホットキー (Alt+Shift+S)
    selector_hotkey: HotKey,
    /// 通知有効・無効切替用ホットキー (Alt+Shift+N)
    notification_hotkey: HotKey,
    /// 一時停止・再開用ホットキー (Alt+Shift+P)
    pause_hotkey: HotKey,
    /// アプリケーション終了用ホットキー (Alt+Shift+Q)
    quit_hotkey: HotKey,
}

// ======================================================================
// 初期化・登録
// ======================================================================
impl HotkeyHandler {
    /// ホットキーハンドラを初期化し、各種ショートカットをシステムに登録する
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `HotkeyHandler` インスタンス。登録に失敗した場合はエラーを返します。
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;
        let selector_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS);
        let notification_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyN);
        let pause_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyP);
        let quit_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyQ);

        let register = |hotkey| manager.register(hotkey).map_err(|e| anyhow::anyhow!(e));

        register(selector_hotkey)?;
        register(notification_hotkey)?;
        register(pause_hotkey)?;
        register(quit_hotkey)?;

        Ok(Self {
            _manager: manager,
            selector_hotkey,
            notification_hotkey,
            pause_hotkey,
            quit_hotkey,
        })
    }
}

// ======================================================================
// イベントリスナー
// ======================================================================
impl HotkeyHandler {
    /// ホットキーイベントを受信するためのバックグラウンドスレッドを開始する
    ///
    /// 受信したイベントは `proxy` を介してメインのイベントループへ転送されます。
    ///
    /// # Arguments
    /// * `proxy` - UIスレッド（イベントループ）へイベントを送信するためのプロキシ
    pub fn start_event_listener(&self, proxy: EventLoopProxy<AppEvent>) {
        std::thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                let _ = proxy.send_event(AppEvent::Hotkey(event));
            }
        });
    }
}

// ======================================================================
// イベント処理
// ======================================================================
impl HotkeyHandler {
    /// 受信したホットキーイベントを解析し、対応するアクションを実行する
    ///
    /// # Arguments
    /// * `event` - 受信したホットキーイベント
    /// * `state` - アプリケーションの共有状態
    /// * `menu` - トレイメニュー構造体
    /// * `selector` - セレクタウィンドウのインスタンス
    /// * `control_flow` - イベントループの制御フロー
    /// * `last_selector_show` - セレクタが最後に表示された時刻（更新用）
    pub fn handle_event(
        &self,
        event: GlobalHotKeyEvent,
        state: &Arc<AppState>,
        menu: &TrayMenu,
        selector: &SelectorWindow,
        control_flow: &mut ControlFlow,
        last_selector_show: &mut Instant,
    ) {
        if event.state == global_hotkey::HotKeyState::Pressed {
            if event.id == self.selector_hotkey.id() {
                if selector.is_visible() {
                    selector.hide();
                } else {
                    *last_selector_show = Instant::now();
                    selector.show(state.with_config(|c| c.mode));
                }
            } else if event.id == self.notification_hotkey.id() {
                let new_val = state.with_config_mut(|c| {
                    c.notification_settings.enabled = !c.notification_settings.enabled;
                    c.notification_settings.enabled
                });
                menu.notification.enabled_item.set_checked(new_val);
                menu.notification.content_submenu.set_enabled(new_val);
                state.save_config();
                notification::show_notification(
                    "ショートカット",
                    if new_val {
                        "成功通知を有効にしました"
                    } else {
                        "成功通知を無効にしました"
                    },
                );
            } else if event.id == self.pause_hotkey.id() {
                let new_paused = state.with_config_mut(|c| {
                    c.is_paused = !c.is_paused;
                    c.is_paused
                });
                menu.pause_item.set_checked(new_paused);
                state.save_config();
                notifier::show_pause_notification(state, new_paused, "ショートカット");
                spawn_monitor_thread(Arc::clone(state));
            } else if event.id == self.quit_hotkey.id() {
                *control_flow = ControlFlow::Exit;
            }
        }
    }
}
