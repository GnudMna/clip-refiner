//! トレイメニュー / メニューイベントの単体テスト用ヘルパー

use std::sync::Arc;
use std::sync::mpsc;

use crate::config::AppConfig;
use crate::tray::menu::TrayMenu;
use crate::tray::state::AppState;
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

use super::worker::{stub_worker, test_app_state_arc, test_tray_menu};

// ======================================================================
// テストコンテキスト
// ======================================================================
/// トレイメニューとスタブワーカーを束ねたテスト用コンテキスト
///
/// メニューイベント・パッチ適用など、`AppState` + `TrayMenu` + ワーカーが揃う経路向け
pub struct TrayTestContext {
    state: Arc<AppState>,
    menu: TrayMenu,
    worker: Arc<ClipboardWorkerHandle>,
    worker_rx: mpsc::Receiver<ClipboardCommand>,
}

impl Default for TrayTestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TrayTestContext {
    /// 既定のテスト用 `AppState` でコンテキストを構築する
    ///
    /// # Returns
    /// * `Self` - トレイメニュー検証用コンテキスト
    pub fn new() -> Self {
        Self::from_state(test_app_state_arc())
    }

    /// メニュー構築前に設定を変更したコンテキストを構築する
    ///
    /// # Arguments
    /// * `configure` - メニュー構築前に `AppConfig` を変更するクロージャ
    ///
    /// # Returns
    /// * `Self` - 変更後の設定でメニューを構築したコンテキスト
    pub fn with_config(configure: impl FnOnce(&mut AppConfig)) -> Self {
        let state = test_app_state_arc();
        state.with_config_mut(configure);
        Self::from_state(state)
    }

    /// 既存の `AppState` からコンテキストを構築する
    ///
    /// 履歴投入など、メニュー構築前に状態を用意したい場合に使う
    ///
    /// # Arguments
    /// * `state` - 共有アプリケーション状態
    ///
    /// # Returns
    /// * `Self` - 指定状態から構築したコンテキスト
    pub fn from_state(state: Arc<AppState>) -> Self {
        let menu = test_tray_menu(&state);
        let (worker, worker_rx) = stub_worker(&state);
        Self {
            state,
            menu,
            worker,
            worker_rx,
        }
    }

    /// 共有 `AppState` への参照を返す
    ///
    /// # Arguments
    /// * `self` - この `TrayTestContext`
    ///
    /// # Returns
    /// * `&Arc<AppState>` - 共有アプリケーション状態
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    /// トレイメニューへの参照を返す
    ///
    /// # Arguments
    /// * `self` - この `TrayTestContext`
    ///
    /// # Returns
    /// * `&TrayMenu` - テスト用トレイメニュー
    pub fn menu(&self) -> &TrayMenu {
        &self.menu
    }

    /// スタブワーカーへの参照を返す
    ///
    /// # Arguments
    /// * `self` - この `TrayTestContext`
    ///
    /// # Returns
    /// * `&Arc<ClipboardWorkerHandle>` - スタブワーカー
    pub fn worker(&self) -> &Arc<ClipboardWorkerHandle> {
        &self.worker
    }

    /// ワーカーコマンド受信側への参照を返す
    ///
    /// # Arguments
    /// * `self` - この `TrayTestContext`
    ///
    /// # Returns
    /// * `&mpsc::Receiver<ClipboardCommand>` - コマンド受信側
    pub fn worker_rx(&self) -> &mpsc::Receiver<ClipboardCommand> {
        &self.worker_rx
    }

    /// 次のワーカーコマンドを受信する
    ///
    /// # Arguments
    /// * `self` - この `TrayTestContext`
    ///
    /// # Returns
    /// * `ClipboardCommand` - 受信したコマンド
    ///
    /// # Panics
    /// 受信に失敗した場合
    pub fn recv_command(&self) -> ClipboardCommand {
        self.worker_rx.recv().expect("ワーカーコマンドが送信される")
    }

    /// ワーカーコマンドが未送信であることを確認する
    ///
    /// # Arguments
    /// * `self` - この `TrayTestContext`
    ///
    /// # Panics
    /// コマンドが既にキューにある場合
    pub fn assert_no_command(&self) {
        assert!(
            self.worker_rx.try_recv().is_err(),
            "ワーカーコマンドが送信されていないこと"
        );
    }
}
