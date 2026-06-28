use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::clipboard_monitor::bump_monitor_generation;
use super::dispatch;
use super::event::update_refine;
use super::menu::TrayMenu;
use super::notify;
#[cfg(windows)]
use super::ocr_capture::OcrCaptureWindow;
use super::quick_selector::QuickSelectorWindow;
use super::state::{AppEvent, AppState};
use super::text_selector::TextSelectorWindow;
use super::worker::ClipboardCommand;
use crate::config::{AppConfig, HotkeySettings};
use crate::consts;
use crate::hotkey_binding::resolve_hotkey;
use crate::platform;
use crate::refiner::RefineMode;

use anyhow::Result;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::HotKey};
use tao::event_loop::{ControlFlow, EventLoopProxy};

// ======================================================================
// ホットキーハンドラ構造体
// ======================================================================
/// お気に入り変換モード用ホットキー割り当て
struct FavoriteHotkeyBinding {
    /// 登録済みホットキー
    hotkey: HotKey,
    /// `favorite_modes` 内のインデックス
    slot_index: usize,
}

/// グローバルホットキーの登録と管理を行う構造体
///
/// アプリケーションが非アクティブな状態でも、特定のキー入力を監視して
/// モード選択UIの表示や設定の切り替えなどを実行する
pub struct HotkeyHandler {
    /// ホットキーマネージャーのインスタンス
    manager: GlobalHotKeyManager,
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
    /// 画面範囲選択 OCR 用ホットキー
    #[cfg(windows)]
    ocr_hotkey: HotKey,
    /// お気に入り変換モード用ホットキー
    favorite_hotkeys: Vec<FavoriteHotkeyBinding>,
}

// ======================================================================
// 初期化・登録
// ======================================================================
/// 解決済みホットキー割り当て
struct ResolvedHotkeys {
    quick_selector: HotKey,
    notification: HotKey,
    pause: HotKey,
    quit: HotKey,
    undo: HotKey,
    text_selector: HotKey,
    #[cfg(windows)]
    ocr: HotKey,
}

impl ResolvedHotkeys {
    /// 設定から各ホットキーを解決する
    fn from_settings(hotkeys: &HotkeySettings) -> Self {
        Self {
            quick_selector: resolve_hotkey(
                &hotkeys.quick_selector,
                consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
                "quick_selector",
            ),
            notification: resolve_hotkey(
                &hotkeys.notification,
                consts::DEFAULT_HOTKEY_NOTIFICATION,
                "notification",
            ),
            pause: resolve_hotkey(&hotkeys.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause"),
            quit: resolve_hotkey(&hotkeys.quit, consts::DEFAULT_HOTKEY_QUIT, "quit"),
            undo: resolve_hotkey(&hotkeys.undo, consts::DEFAULT_HOTKEY_UNDO, "undo"),
            text_selector: resolve_hotkey(
                &hotkeys.text_selector,
                consts::DEFAULT_HOTKEY_TEXT_SELECTOR,
                "text_selector",
            ),
            #[cfg(windows)]
            ocr: resolve_hotkey(&hotkeys.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr"),
        }
    }

    /// 登録済みホットキーを配列として返す
    fn registered_hotkeys(&self) -> Vec<HotKey> {
        let mut hotkeys = vec![
            self.quick_selector,
            self.notification,
            self.pause,
            self.quit,
            self.undo,
            self.text_selector,
        ];
        #[cfg(windows)]
        hotkeys.push(self.ocr);
        hotkeys
    }
}

impl HotkeyHandler {
    /// ホットキーハンドラを初期化し、各種ショートカットをシステムに登録する
    ///
    /// # Arguments
    /// * `hotkeys` - 設定ファイルから読み込んだホットキー割り当て
    /// * `favorite_modes` - お気に入り登録済み変換モード
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `HotkeyHandler` インスタンス。登録に失敗した場合はエラーを返す
    pub fn new(hotkeys: &HotkeySettings, favorite_modes: &[RefineMode]) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;
        let resolved = ResolvedHotkeys::from_settings(hotkeys);

        for hotkey in resolved.registered_hotkeys() {
            manager.register(hotkey).map_err(|e| anyhow::anyhow!(e))?;
        }

        let mut handler = Self {
            manager,
            quick_selector_hotkey: resolved.quick_selector,
            notification_hotkey: resolved.notification,
            pause_hotkey: resolved.pause,
            quit_hotkey: resolved.quit,
            undo_hotkey: resolved.undo,
            text_selector_hotkey: resolved.text_selector,
            #[cfg(windows)]
            ocr_hotkey: resolved.ocr,
            favorite_hotkeys: Vec::new(),
        };
        handler.register_favorite_hotkeys(hotkeys, favorite_modes.len())?;
        Ok(handler)
    }

    /// ホットキー割り当てを再登録する
    ///
    /// 設定ファイルの再読み込み後など、再起動なしでショートカットを反映する
    ///
    /// # Arguments
    /// * `hotkeys` - 新しいホットキー割り当て
    /// * `favorite_modes` - お気に入り登録済み変換モード
    ///
    /// # Returns
    /// * `Result<()>` - 再登録成功時は `Ok(())`、失敗時は `Err`
    pub fn reload(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_modes: &[RefineMode],
    ) -> Result<()> {
        for hotkey in self.registered_hotkeys() {
            self.manager
                .unregister(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        self.unregister_favorite_hotkeys()?;

        let resolved = ResolvedHotkeys::from_settings(hotkeys);
        for hotkey in resolved.registered_hotkeys() {
            self.manager
                .register(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        self.quick_selector_hotkey = resolved.quick_selector;
        self.notification_hotkey = resolved.notification;
        self.pause_hotkey = resolved.pause;
        self.quit_hotkey = resolved.quit;
        self.undo_hotkey = resolved.undo;
        self.text_selector_hotkey = resolved.text_selector;
        #[cfg(windows)]
        {
            self.ocr_hotkey = resolved.ocr;
        }

        self.register_favorite_hotkeys(hotkeys, favorite_modes.len())
    }

    /// お気に入り変換モード用ホットキーのみ再登録する
    pub fn reload_favorite_slots(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_count: usize,
    ) -> Result<()> {
        self.unregister_favorite_hotkeys()?;
        self.register_favorite_hotkeys(hotkeys, favorite_count)
    }

    /// 登録済みホットキーを配列として返す
    fn registered_hotkeys(&self) -> Vec<HotKey> {
        let mut hotkeys = vec![
            self.quick_selector_hotkey,
            self.notification_hotkey,
            self.pause_hotkey,
            self.quit_hotkey,
            self.undo_hotkey,
            self.text_selector_hotkey,
        ];
        #[cfg(windows)]
        hotkeys.push(self.ocr_hotkey);
        hotkeys
    }

    /// お気に入り変換モード用ホットキーを OS へ登録する
    fn register_favorite_hotkeys(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_count: usize,
    ) -> Result<()> {
        let resolved =
            hotkeys.resolve_favorite_slot_hotkeys(favorite_count, &self.registered_hotkeys());
        for (slot_index, hotkey) in resolved {
            self.manager
                .register(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
            self.favorite_hotkeys
                .push(FavoriteHotkeyBinding { hotkey, slot_index });
        }
        Ok(())
    }

    /// お気に入り変換モード用ホットキーの登録を解除する
    fn unregister_favorite_hotkeys(&mut self) -> Result<()> {
        for binding in &self.favorite_hotkeys {
            self.manager
                .unregister(binding.hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        self.favorite_hotkeys.clear();
        Ok(())
    }

    /// ホットキー ID に対応するお気に入りスロットインデックスを返す
    fn favorite_slot_for_hotkey(&self, hotkey_id: u32) -> Option<usize> {
        self.favorite_hotkeys
            .iter()
            .find(|binding| binding.hotkey.id() == hotkey_id)
            .map(|binding| binding.slot_index)
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
                dispatch::send_app_event(&proxy, AppEvent::Hotkey(event));
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
    /// OCR キャプチャオーバーレイ (OCR 操作時のみ必要)
    #[cfg(windows)]
    pub ocr_capture: Option<&'a OcrCaptureWindow>,
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
            dispatch::send_clipboard_command(ctx.clipboard_tx, ClipboardCommand::Undo);
        } else if event.id == self.text_selector_hotkey.id() {
            Self::handle_text_selector_hotkey(ctx);
        } else if self.handle_ocr_hotkey_if_pressed(event.id, ctx) {
        } else if let Some(slot_index) = self.favorite_slot_for_hotkey(event.id) {
            Self::handle_favorite_mode_hotkey(ctx, slot_index);
        }
    }

    /// お気に入り変換モード用ホットキーを処理する
    fn handle_favorite_mode_hotkey(ctx: &mut HotkeyEventContext<'_>, slot_index: usize) {
        let Some(mode) = ctx
            .state
            .with_config(|config| config.favorite_modes.get(slot_index).copied())
        else {
            return;
        };

        if let Some(quick_selector) = ctx.quick_selector {
            quick_selector.hide();
        }
        if let Some(text_selector) = ctx.text_selector
            && text_selector.is_visible()
        {
            text_selector.hide();
        }

        update_refine(
            ctx.state,
            ctx.menu,
            ctx.clipboard_tx,
            mode,
            ctx.quick_selector,
        );
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
            let modes_json = ctx.state.with_config(AppConfig::modes_to_json_list);
            quick_selector.show(ctx.state.with_config(|c| c.mode), &modes_json);
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

    /// 画面 OCR キャプチャオーバーレイ表示ホットキーを処理する
    #[cfg(windows)]
    fn handle_ocr_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(ocr_capture) = ctx.ocr_capture else {
            return;
        };

        if ocr_capture.is_visible() {
            ocr_capture.hide();
            return;
        }

        if let Some(quick_selector) = ctx.quick_selector
            && quick_selector.is_visible()
        {
            quick_selector.hide();
        }
        if let Some(text_selector) = ctx.text_selector
            && text_selector.is_visible()
        {
            text_selector.hide();
        }

        ocr_capture.show();
    }

    /// OCR ホットキーが押された場合に処理する
    #[cfg(windows)]
    fn handle_ocr_hotkey_if_pressed(
        &self,
        hotkey_id: u32,
        ctx: &mut HotkeyEventContext<'_>,
    ) -> bool {
        if hotkey_id != self.ocr_hotkey.id() {
            return false;
        }
        Self::handle_ocr_hotkey(ctx);
        true
    }

    #[cfg(not(windows))]
    fn handle_ocr_hotkey_if_pressed(
        &self,
        _hotkey_id: u32,
        _ctx: &mut HotkeyEventContext<'_>,
    ) -> bool {
        false
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

    /// テスト用: お気に入りスロット 0 のホットキー ID を返す
    pub(crate) fn favorite_hotkey_id_at(&self, slot_index: usize) -> Option<u32> {
        self.favorite_hotkeys
            .iter()
            .find(|binding| binding.slot_index == slot_index)
            .map(|binding| binding.hotkey.id())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Instant;

    use super::*;

    use crate::config::HotkeySettings;
    use crate::refiner::RefineMode;
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
                #[cfg(windows)]
                ocr_capture: None,
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
            ocr: "Alt+Shift+F7".to_string(),
            favorite_mode_slots: Vec::new(),
        }
    }

    /// ホットキーイベント処理の一連の挙動を検証する
    ///
    /// グローバルホットキーは OS に残るため 1 テストにまとめる
    #[test]
    fn hotkey_event_handling() {
        let mut handler =
            HotkeyHandler::new(&test_hotkeys(), &[]).expect("テスト用ホットキーの登録に失敗");
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

        // お気に入りホットキー
        ctx.state.with_config_mut(|config| {
            config.favorite_modes = vec![RefineMode::Trim, RefineMode::JsonFormat];
        });
        handler
            .reload_favorite_slots(&test_hotkeys(), 2)
            .expect("お気に入りホットキーの再登録に失敗");
        let favorite_id = handler
            .favorite_hotkey_id_at(1)
            .expect("スロット 1 のホットキーが登録される");
        handler.handle_event(
            GlobalHotKeyEvent {
                id: favorite_id,
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert_eq!(
            ctx.state.with_config(|config| config.mode),
            RefineMode::JsonFormat
        );
        assert!(matches!(
            rx.recv()
                .expect("お気に入りホットキーで加工コマンドが送信される"),
            ClipboardCommand::ProcessMode(RefineMode::JsonFormat)
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

    /// `reload` でホットキー ID が更新されること
    #[test]
    fn reload_updates_registered_hotkey_ids() {
        let initial = HotkeySettings {
            quick_selector: "Alt+Ctrl+F1".to_string(),
            notification: "Alt+Ctrl+F2".to_string(),
            pause: "Alt+Ctrl+F3".to_string(),
            quit: "Alt+Ctrl+F4".to_string(),
            undo: "Alt+Ctrl+F5".to_string(),
            text_selector: "Alt+Ctrl+F6".to_string(),
            ocr: "Alt+Ctrl+F7".to_string(),
            favorite_mode_slots: Vec::new(),
        };
        let mut handler =
            HotkeyHandler::new(&initial, &[]).expect("テスト用ホットキーの登録に失敗");
        let quit_before = handler.quit_hotkey_id();

        let mut updated = initial.clone();
        updated.quit = "Alt+Ctrl+F12".to_string();
        handler
            .reload(&updated, &[])
            .expect("ホットキーの再登録に失敗");

        assert_ne!(handler.quit_hotkey_id(), quit_before);
    }
}
