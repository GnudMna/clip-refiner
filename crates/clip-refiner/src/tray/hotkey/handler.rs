use std::sync::Arc;
use std::time::Instant;

use global_hotkey::GlobalHotKeyEvent;
use tao::event_loop::{ControlFlow, EventLoopWindowTarget};

use super::super::dispatch;
use super::super::event::apply_refine_once;
use super::super::event::set_auto_refine_enabled as apply_auto_refine_enabled;
use super::super::event::toggle_continuous_record as apply_continuous_record_toggle;
use super::super::event::undo_last_continuous_record as apply_continuous_record_undo;
use super::super::menu::TrayMenu;
use super::super::ocr::OcrCaptureWindow;
use super::super::selector::{
    ClipSelectorWindow, QuickSelectorWindow, clip_selector_json, hide_with_retention,
    quick_selector_current_mode, quick_selector_json,
};
use super::super::state::{AppState, SettingsEvent};
use super::super::worker::{ClipboardCommand, ClipboardWorkerHandle};
use super::register::HotkeyHandler;
use crate::platform;
use crate::tray::state::AppEvent;

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
    /// 登録クリップセレクターウィンドウのインスタンス (登録クリップセレクター操作時のみ必要)
    pub clip_selector: Option<&'a ClipSelectorWindow>,
    /// OCR キャプチャオーバーレイ (OCR 操作時のみ必要)
    pub ocr_capture: Option<&'a OcrCaptureWindow>,
    /// 遅延ウィンドウ生成用のイベントループ (セレクター表示時に必要)
    pub event_loop: Option<&'a EventLoopWindowTarget<AppEvent>>,
    /// イベントループの制御フロー
    pub control_flow: &'a mut ControlFlow,
    /// クイックセレクターが最後に表示された時刻(更新用)
    pub last_quick_selector_show: &'a mut Instant,
    /// 登録クリップセレクターが最後に表示された時刻(更新用)
    pub last_clip_selector_show: &'a mut Instant,
    /// クリップボード・ワーカー
    pub clipboard_worker: &'a ClipboardWorkerHandle,
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
        } else if event.id == self.auto_refine_hotkey.id() {
            Self::toggle_auto_refine(ctx);
        } else if event.id == self.continuous_record_hotkey.id() {
            Self::toggle_continuous_record(ctx);
        } else if event.id == self.continuous_record_undo_hotkey.id() {
            Self::undo_last_continuous_record(ctx);
        } else if event.id == self.continuous_record_paste_hotkey.id() {
            Self::paste_continuous_record(ctx);
        } else if event.id == self.quit_hotkey.id() {
            *ctx.control_flow = ControlFlow::Exit;
        } else if event.id == self.undo_hotkey.id() {
            dispatch::send_clipboard_command(ctx.clipboard_worker, ClipboardCommand::Undo);
        } else if event.id == self.clip_selector_hotkey.id() {
            Self::handle_clip_selector_hotkey(ctx);
        } else if self.handle_ocr_hotkey_if_pressed(event.id, ctx) {
        } else if let Some(slot_index) = self.favorite_slot_for_hotkey(event.id) {
            Self::handle_favorite_mode_hotkey(ctx, slot_index);
        }
    }

    /// お気に入り変換モード用ホットキーを処理する
    ///
    /// 自動変換の設定は変えず、現在のクリップボードへ一回だけ適用する
    fn handle_favorite_mode_hotkey(ctx: &mut HotkeyEventContext<'_>, slot_index: usize) {
        let Some(mode) = ctx
            .state
            .with_config(|config| config.favorite_modes.get(slot_index).copied())
        else {
            return;
        };

        if let Some(quick_selector) = ctx.quick_selector {
            let retention = ctx.state.with_config(|c| c.selector_webview_retention);
            hide_with_retention(quick_selector, retention);
        }
        if let Some(clip_selector) = ctx.clip_selector
            && clip_selector.is_visible()
        {
            let retention = ctx.state.with_config(|c| c.selector_webview_retention);
            hide_with_retention(clip_selector, retention);
        }

        apply_refine_once(ctx.clipboard_worker, mode);
    }

    /// クイックセレクター表示ホットキーを処理する
    fn handle_quick_selector_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(quick_selector) = ctx.quick_selector else {
            return;
        };
        let retention = ctx.state.with_config(|c| c.selector_webview_retention);

        if quick_selector.is_visible() {
            hide_with_retention(quick_selector, retention);
        } else {
            if let Some(clip_selector) = ctx.clip_selector
                && clip_selector.is_visible()
            {
                hide_with_retention(clip_selector, retention);
            }
            *ctx.last_quick_selector_show = Instant::now();
            let Some(event_loop) = ctx.event_loop else {
                return;
            };
            let modes_json = ctx.state.with_config(quick_selector_json);
            quick_selector.show(
                event_loop,
                ctx.state.with_config(quick_selector_current_mode),
                &modes_json,
            );
        }
    }

    /// 登録クリップセレクター表示ホットキーを処理する
    fn handle_clip_selector_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(clip_selector) = ctx.clip_selector else {
            return;
        };
        let retention = ctx.state.with_config(|c| c.selector_webview_retention);

        if clip_selector.is_visible() {
            hide_with_retention(clip_selector, retention);
        } else {
            if let Some(quick_selector) = ctx.quick_selector
                && quick_selector.is_visible()
            {
                hide_with_retention(quick_selector, retention);
            }
            *ctx.last_clip_selector_show = Instant::now();
            let Some(event_loop) = ctx.event_loop else {
                return;
            };
            let clips_json = ctx.state.with_config(clip_selector_json);
            clip_selector.show(event_loop, &clips_json);
        }
    }

    /// 画面 OCR キャプチャオーバーレイ表示ホットキーを処理する
    fn handle_ocr_hotkey(ctx: &mut HotkeyEventContext<'_>) {
        let Some(ocr_capture) = ctx.ocr_capture else {
            return;
        };

        if ocr_capture.is_visible() {
            ocr_capture.hide();
            return;
        }

        let retention = ctx.state.with_config(|c| c.selector_webview_retention);
        if let Some(quick_selector) = ctx.quick_selector
            && quick_selector.is_visible()
        {
            hide_with_retention(quick_selector, retention);
        }
        if let Some(clip_selector) = ctx.clip_selector
            && clip_selector.is_visible()
        {
            hide_with_retention(clip_selector, retention);
        }

        ocr_capture.show();
    }

    /// OCR ホットキーが押された場合に処理する
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

    /// 成功通知の有効/無効を切り替える
    pub(super) fn toggle_notification(ctx: &mut HotkeyEventContext<'_>) {
        let new_val = ctx.state.with_config_mut(|c| {
            c.notification_settings.enabled = !c.notification_settings.enabled;
            c.notification_settings.enabled
        });
        ctx.state.save_config();
        let _ = ctx.state.proxy.send_event(SettingsEvent::Refresh.into());
        platform::show_notification(
            "ショートカット",
            if new_val {
                "成功通知を有効にしました"
            } else {
                "成功通知を無効にしました"
            },
        );
    }

    /// 自動変換の有効/無効を切り替える
    ///
    /// 有効にするときに連続記録が有効なら、連続記録を無効にする
    pub(super) fn toggle_auto_refine(ctx: &mut HotkeyEventContext<'_>) {
        let enabled = !ctx.state.with_config(|c| c.auto_refine_enabled);
        apply_auto_refine_enabled(ctx.state, ctx.menu, enabled, Some("ショートカット"));
        ctx.state.save_config();
        let _ = ctx.state.proxy.send_event(SettingsEvent::Refresh.into());
    }

    /// 連続記録の有効/無効を切り替える
    pub(super) fn toggle_continuous_record(ctx: &mut HotkeyEventContext<'_>) {
        apply_continuous_record_toggle(
            ctx.menu,
            ctx.state,
            ctx.clipboard_worker,
            Some("ショートカット"),
        );
    }

    /// 連続記録の直近 1 件を取り消す
    pub(super) fn undo_last_continuous_record(ctx: &mut HotkeyEventContext<'_>) {
        apply_continuous_record_undo(ctx.menu, ctx.state, "ショートカット");
    }

    /// 連続記録キュー先頭の貼り付けをワーカーへ依頼する
    pub(super) fn paste_continuous_record(ctx: &mut HotkeyEventContext<'_>) {
        dispatch::send_clipboard_command(
            ctx.clipboard_worker,
            ClipboardCommand::PasteContinuousRecordFront,
        );
    }
}
