use std::sync::Arc;

use super::super::apply::{
    SettingsApplyFeedback, SettingsWindowCommand, handle_settings_window_command,
};
use super::super::ipc::SettingsPatch;
use super::super::regex_preview::RegexPreviewResponse;
use super::SettingsWindow;
use super::open_settings_window;

use crate::platform::update::UpdateCheckSource;
use crate::tray::features::{start_update_apply, start_update_check};
use crate::tray::menu::TrayMenu;
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::state::{AppEvent, AppState};
use crate::tray::worker::ClipboardWorkerHandle;

use tao::event_loop::EventLoopWindowTarget;

// ======================================================================
// イベント処理
// ======================================================================
impl SettingsWindow {
    /// 設定ウィンドウ関連の `AppEvent` を処理する
    pub(crate) fn handle_event(
        &self,
        event: AppEvent,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        state: &Arc<AppState>,
        menu: &TrayMenu,
        quick_selector: Option<&QuickSelectorWindow>,
        clipboard_worker: &ClipboardWorkerHandle,
    ) {
        match event {
            AppEvent::ShowSettingsWindow => open_settings_window(event_loop, state, self),
            AppEvent::HideSettingsWindow => self.hide(),
            AppEvent::SettingsApplyPatch => {
                if let Some(json) = state.settings_pending().take_patch() {
                    match serde_json::from_str::<SettingsPatch>(&json) {
                        Ok(patch) => handle_settings_window_command(
                            SettingsWindowCommand::Apply(Box::new(patch)),
                            state,
                            menu,
                            self,
                            quick_selector,
                            clipboard_worker,
                        ),
                        Err(err) => {
                            crate::log_warn!("設定パッチの JSON 解釈に失敗: {:?}", err);
                            self.notify_apply_feedback(&SettingsApplyFeedback::error(
                                None,
                                "設定の適用に失敗しました",
                            ));
                        }
                    }
                }
            }
            AppEvent::SettingsClearHistory => {
                handle_settings_window_command(
                    SettingsWindowCommand::ClearHistory,
                    state,
                    menu,
                    self,
                    quick_selector,
                    clipboard_worker,
                );
            }
            AppEvent::SettingsClearContinuousRecord => {
                handle_settings_window_command(
                    SettingsWindowCommand::ClearContinuousRecord,
                    state,
                    menu,
                    self,
                    quick_selector,
                    clipboard_worker,
                );
            }
            AppEvent::SettingsOpenConfigFile => {
                handle_settings_window_command(
                    SettingsWindowCommand::OpenConfigFile,
                    state,
                    menu,
                    self,
                    quick_selector,
                    clipboard_worker,
                );
            }
            AppEvent::SettingsRegexPreview => {
                if let Some(request) = state.settings_pending().take_regex_preview_request() {
                    let response = RegexPreviewResponse::from_request(&request);
                    self.notify_regex_preview(&response);
                }
            }
            AppEvent::SettingsCheckUpdate => {
                let _ = start_update_check(state, UpdateCheckSource::Settings, Some(self));
            }
            AppEvent::SettingsApplyUpdate => {
                let _ = start_update_apply(state, self);
            }
            _ => {}
        }
    }
}
