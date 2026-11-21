//! 設定ウィンドウ IPC / パッチ適用の統合テスト用ヘルパー

use std::sync::Arc;
use std::sync::mpsc;

use crate::tray::menu::TrayMenu;
use crate::tray::settings::test_support::{
    SettingsIpcAction, SettingsPatch, apply_settings_patch_without_ui, enqueue_settings_patch,
    enqueue_settings_patch_json, parse_settings_ipc_message,
};
use crate::tray::state::{AppState, test_app_state};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

// ======================================================================
// テストコンテキスト
// ======================================================================
/// 設定ウィンドウの IPC キューとパッチ適用を検証するためのコンテキスト
pub struct SettingsTestContext {
    state: Arc<AppState>,
    menu: TrayMenu,
    worker: Arc<ClipboardWorkerHandle>,
    _worker_rx: mpsc::Receiver<ClipboardCommand>,
}

impl Default for SettingsTestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsTestContext {
    /// テスト用 `AppState`・トレイメニュー・スタブワーカーを構築する
    pub fn new() -> Self {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, rx) = test_worker(&state);
        Self {
            state,
            menu,
            worker,
            _worker_rx: rx,
        }
    }

    /// 共有 `AppState` への参照を返す
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    /// トレイメニューへの参照を返す
    pub fn menu(&self) -> &TrayMenu {
        &self.menu
    }

    /// JSON IPC メッセージの `apply` を適用待ちキューへ積む
    pub fn enqueue_apply_ipc(&self, json: &str) -> bool {
        let Some(SettingsIpcAction::Apply(patch)) = parse_settings_ipc_message(json) else {
            return false;
        };
        enqueue_settings_patch(&self.state, &patch)
    }

    /// JSON オブジェクト形式の部分更新を適用待ちキューへ積む
    pub fn enqueue_patch_json(&self, patch_json: &str) -> bool {
        enqueue_settings_patch_json(&self.state, patch_json)
    }

    /// 適用待ちキューからマージ済みパッチ JSON を取り出す
    pub fn take_pending_patch_json(&self) -> Option<String> {
        self.state.take_pending_settings_patch()
    }

    /// 適用待ちキューからパッチを取り出して反映する
    pub fn apply_pending_patch(&self) -> bool {
        let Some(json) = self.take_pending_patch_json() else {
            return false;
        };
        let patch: SettingsPatch = match serde_json::from_str(&json) {
            Ok(patch) => patch,
            Err(_) => return false,
        };
        self.apply_patch(&patch);
        true
    }

    /// 部分更新パッチを設定へ反映する
    fn apply_patch(&self, patch: &SettingsPatch) {
        apply_settings_patch_without_ui(patch, &self.state, &self.menu, None, &self.worker);
    }

    /// JSON IPC の `apply` を解釈して即時反映する
    pub fn apply_ipc(&self, json: &str) -> bool {
        let Some(SettingsIpcAction::Apply(patch)) = parse_settings_ipc_message(json) else {
            return false;
        };
        self.apply_patch(&patch);
        true
    }
}

fn test_worker(
    state: &Arc<AppState>,
) -> (Arc<ClipboardWorkerHandle>, mpsc::Receiver<ClipboardCommand>) {
    let (tx, rx) = mpsc::channel();
    let worker = ClipboardWorkerHandle::for_test(Arc::clone(state), tx);
    (worker, rx)
}
