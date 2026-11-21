//! 設定ウィンドウ IPC / パッチ適用の統合テスト用ヘルパー

use std::sync::Arc;
use std::sync::mpsc;

use super::worker::{stub_worker, test_tray_menu};
use crate::tray::menu::TrayMenu;
use crate::tray::settings::test_support::{
    SettingsIpcAction, SettingsPatch, apply_settings_patch_without_ui, enqueue_settings_patch,
    enqueue_settings_patch_json, parse_settings_ipc_message,
};
use crate::tray::state::{AppState, test_app_state, test_app_state_with_persistence};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

use clip_refiner_core::test_helpers::with_temp_config_dir;

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
    ///
    /// # Returns
    /// * `Self` - 設定 IPC テスト用コンテキスト
    pub fn new() -> Self {
        Self::from_state(Arc::new(test_app_state()))
    }

    /// ディスク永続化を有効にしたテストコンテキストを構築する
    ///
    /// `with_temp_config_dir` / `with_persistence` 内でのみ使うこと
    ///
    /// # Returns
    /// * `Self` - 設定の保存・再読込を検証できるコンテキスト
    pub fn new_with_persistence() -> Self {
        Self::from_state(Arc::new(test_app_state_with_persistence()))
    }

    /// 一時設定ディレクトリ上で永続化付きコンテキストを実行する
    ///
    /// # Arguments
    /// * `f` - 永続化付きコンテキストを受け取るクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャの戻り値
    pub fn with_persistence<F, R>(f: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        with_temp_config_dir(|| {
            let ctx = Self::new_with_persistence();
            f(&ctx)
        })
    }

    fn from_state(state: Arc<AppState>) -> Self {
        let menu = test_tray_menu(&state);
        let (worker, rx) = stub_worker(&state);
        Self {
            state,
            menu,
            worker,
            _worker_rx: rx,
        }
    }

    /// 共有 `AppState` への参照を返す
    ///
    /// # Arguments
    /// * `self` - この `SettingsTestContext`
    ///
    /// # Returns
    /// * `&Arc<AppState>` - 共有アプリケーション状態
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    /// トレイメニューへの参照を返す
    ///
    /// # Arguments
    /// * `self` - この `SettingsTestContext`
    ///
    /// # Returns
    /// * `&TrayMenu` - テスト用トレイメニュー
    pub fn menu(&self) -> &TrayMenu {
        &self.menu
    }

    /// 設定の永続化が有効かどうか返す
    ///
    /// # Arguments
    /// * `self` - この `SettingsTestContext`
    ///
    /// # Returns
    /// * `bool` - 永続化が有効なら `true`
    pub fn is_config_persistence_enabled(&self) -> bool {
        self.state.config_sync().is_persistence_enabled()
    }

    /// JSON IPC メッセージの `apply` を適用待ちキューへ積む
    ///
    /// # Arguments
    /// * `json` - 設定ウィンドウから送られる JSON IPC メッセージ
    ///
    /// # Returns
    /// * `bool` - `apply` を解釈してキューへ積めた場合は `true`
    pub fn enqueue_apply_ipc(&self, json: &str) -> bool {
        let Some(SettingsIpcAction::Apply(patch)) = parse_settings_ipc_message(json) else {
            return false;
        };
        enqueue_settings_patch(&self.state, &patch)
    }

    /// JSON オブジェクト形式の部分更新を適用待ちキューへ積む
    ///
    /// # Arguments
    /// * `patch_json` - 部分更新パッチの JSON 文字列
    ///
    /// # Returns
    /// * `bool` - パッチを解釈してキューへ積めた場合は `true`
    pub fn enqueue_patch_json(&self, patch_json: &str) -> bool {
        enqueue_settings_patch_json(&self.state, patch_json)
    }

    /// 適用待ちキューからマージ済みパッチ JSON を取り出す
    ///
    /// # Arguments
    /// * `self` - この `SettingsTestContext`
    ///
    /// # Returns
    /// * `Option<String>` - キューにパッチがあれば `Some(JSON)`、空なら `None`
    pub fn take_pending_patch_json(&self) -> Option<String> {
        self.state.settings_pending().take_patch()
    }

    /// 適用待ちキューからパッチを取り出して反映する
    ///
    /// # Arguments
    /// * `self` - この `SettingsTestContext`
    ///
    /// # Returns
    /// * `bool` - パッチを取り出して反映できた場合は `true`
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
    ///
    /// # Arguments
    /// * `json` - 設定ウィンドウから送られる JSON IPC メッセージ
    ///
    /// # Returns
    /// * `bool` - `apply` を解釈して反映できた場合は `true`
    pub fn apply_ipc(&self, json: &str) -> bool {
        let Some(SettingsIpcAction::Apply(patch)) = parse_settings_ipc_message(json) else {
            return false;
        };
        self.apply_patch(&patch);
        true
    }
}
