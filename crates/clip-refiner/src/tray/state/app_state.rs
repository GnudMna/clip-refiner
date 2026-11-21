use std::sync::{RwLock, atomic::AtomicU64};

use super::super::dispatch;
use super::app_event::AppEvent;
use super::clipboard_session::ClipboardSessionState;
use super::config_sync::ConfigSyncState;
use super::lock_ext::RwLockExt;
use super::monitor_snapshot::{MonitorSnapshot, ProcessedState};
use super::settings_pending::{RegexPreviewRequest, SettingsPendingState};
use super::worker_state::WorkerState;

use crate::config::{AppConfig, ConfigLoadFallback};
use crate::security::SecretString;

use tao::event_loop::EventLoopProxy;

// ======================================================================
// アプリケーション状態
// ======================================================================
/// アプリケーション全体で共有され、スレッド間で安全に読み書きされる状態管理構造体
///
/// 永続設定・ワーカー稼働・監視世代・セッション揮発データ・設定ウィンドウ適用待ちを保持する
pub struct AppState {
    worker: WorkerState,
    /// 永続化設定データ
    pub config: RwLock<AppConfig>,
    /// クリップボード監視スレッドの世代管理カウンタ
    pub monitor_generation: AtomicU64,
    clipboard_session: ClipboardSessionState,
    config_sync: ConfigSyncState,
    settings_pending: SettingsPendingState,
    /// メインのイベントループへメッセージを送るためのプロキシ
    pub proxy: EventLoopProxy<AppEvent>,
}

// ======================================================================
// 初期化・設定保存
// ======================================================================
impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Arguments
    /// * `proxy` - メインイベントループへ通知を送るプロキシ
    ///
    /// # Returns
    /// * `Result<(Self, Option<ConfigLoadFallback>)>` - 生成された `AppState` と、設定読み込み失敗時のフォールバック理由
    pub fn new(
        proxy: EventLoopProxy<AppEvent>,
    ) -> anyhow::Result<(Self, Option<ConfigLoadFallback>)> {
        let load_outcome = AppConfig::load();
        let config = load_outcome.config;
        Ok((
            Self {
                worker: WorkerState::new(),
                config: RwLock::new(config),
                monitor_generation: AtomicU64::new(0),
                clipboard_session: ClipboardSessionState::new()?,
                config_sync: ConfigSyncState::new(),
                settings_pending: SettingsPendingState::new(),
                proxy,
            },
            load_outcome.fallback,
        ))
    }

    /// 適用待ちの部分更新 JSON キューへ排他アクセスする
    pub(crate) fn pending_settings_patch_mut(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.settings_pending.patch_mut()
    }

    /// キュー済みの部分更新 JSON を取り出す
    pub(crate) fn take_pending_settings_patch(&self) -> Option<String> {
        self.settings_pending.take_patch()
    }

    /// 正規表現プレビュー要求を格納する
    pub(crate) fn set_regex_preview_request(&self, request: RegexPreviewRequest) {
        self.settings_pending.set_regex_preview_request(request);
    }

    /// 処理待ちの正規表現プレビュー要求を取り出す
    pub(crate) fn take_regex_preview_request(&self) -> Option<RegexPreviewRequest> {
        self.settings_pending.take_regex_preview_request()
    }

    /// ディスク上の設定ファイルと同期済みの更新時刻を記録する
    pub fn record_config_disk_sync(&self) {
        self.config_sync.record_disk_sync();
    }

    /// 外部編集による設定ファイルの変更を検知する
    ///
    /// アプリ自身の保存直後は同期済み時刻と一致するため `false` を返す
    ///
    /// # Returns
    /// * `bool` - ディスク上の設定が最終同期時刻より新しい場合は `true`
    pub fn has_external_config_change(&self) -> bool {
        self.config_sync.has_external_change()
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// テスト用 `AppState` ではディスクへ書き込まない
    pub fn save_config(&self) {
        if !self.config_sync.should_persist() {
            return;
        }
        if let Err(e) = self.with_config(crate::config::AppConfig::save) {
            crate::log_error!("設定の保存に失敗: {:?}", e);
            return;
        }
        self.config_sync.begin_save_grace();
        self.record_config_disk_sync();
    }

    /// 設定を読み取り専用で参照する
    ///
    /// # Arguments
    /// * `f` - `AppConfig` へ共有参照でアクセスするクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャ `f` の戻り値
    pub fn with_config<R>(&self, f: impl FnOnce(&AppConfig) -> R) -> R {
        f(&self.config.read_ignore_poison())
    }

    /// 設定を変更する
    ///
    /// # Arguments
    /// * `f` - `AppConfig` へ排他アクセスして変更を行うクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャ `f` の戻り値
    pub fn with_config_mut<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> R {
        f(&mut self.config.write_ignore_poison())
    }

    /// クリップボードワーカーが稼働中かどうか
    ///
    /// # Returns
    /// * `bool` - ワーカーが稼働中の場合は `true`
    pub fn is_worker_alive(&self) -> bool {
        self.worker.is_alive()
    }

    /// クリップボードワーカーの稼働状態を更新する
    ///
    /// # Arguments
    /// * `alive` - 稼働中にする場合は `true`
    pub fn set_worker_alive(&self, alive: bool) {
        self.worker.set_alive(alive);
    }

    /// ワーカー復旧成功時の通知が必要であることを記録する
    ///
    /// # Arguments
    /// * `pending` - 通知が必要な場合は `true`
    pub fn set_worker_recovery_pending(&self, pending: bool) {
        self.worker.set_recovery_pending(pending);
    }

    /// ワーカー復旧成功時の通知が必要かどうかを取得し、フラグをクリアする
    ///
    /// # Returns
    /// * `bool` - 通知が必要だった場合は `true`
    pub fn take_worker_recovery_pending(&self) -> bool {
        self.worker.take_recovery_pending()
    }

    /// 設定のディスク保存が有効かどうかを返す
    #[cfg(test)]
    pub(crate) fn is_config_persistence_enabled(&self) -> bool {
        self.config_sync.is_persistence_enabled()
    }
}

// ======================================================================
// 状態操作
// ======================================================================
impl AppState {
    /// 監視ループ向けに設定フィールドをまとめて一括取得する
    ///
    /// `config` `RwLock` の取得を1回に抑えることで、ループ毎の細粒度ロックを削減する
    ///
    /// # Returns
    /// * `MonitorSnapshot` - 監視ループ向け設定スナップショット
    pub fn monitor_snapshot(&self) -> MonitorSnapshot {
        self.with_config(|config| MonitorSnapshot {
            pipeline: config.effective_pipeline(),
            interval_ms: config.interval_ms,
            is_paused: config.is_paused,
            history_enabled: config.history_enabled,
            continuous_record_enabled: config.continuous_record_enabled,
            regex_settings: config.regex.clone(),
        })
    }

    /// 二重加工防止状態を更新する
    ///
    /// # Arguments
    /// * `f` - `ProcessedState` へ排他アクセスして変更を行うクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャ `f` の戻り値
    pub fn with_processed_state<R>(&self, f: impl FnOnce(&mut ProcessedState) -> R) -> R {
        self.clipboard_session.with_processed_state(f)
    }

    /// 加工成功後にクリップボードへ書き戻したことを記録する
    ///
    /// # Arguments
    /// * `output` - 書き戻した加工後テキスト
    pub fn record_processing_success(&self, output: &str) {
        self.clipboard_session.record_processing_success(output);
    }

    /// 画像加工成功後に元テキストの指紋を記録する
    ///
    /// # Arguments
    /// * `source_text` - 加工前の元テキスト
    pub fn record_image_processing_success(&self, source_text: &str) {
        self.clipboard_session
            .record_image_processing_success(source_text);
    }

    /// 加工せずに観測したクリップボード本文を記録する
    ///
    /// # Arguments
    /// * `text` - 観測したクリップボード本文
    pub fn record_clipboard_observed(&self, text: &str) {
        self.clipboard_session.record_clipboard_observed(text);
    }

    /// 履歴復元など、外部からクリップボードへ設定した本文を記録する
    ///
    /// # Arguments
    /// * `text` - クリップボードへ設定した本文
    pub fn record_clipboard_set(&self, text: &str) {
        self.clipboard_session.record_clipboard_set(text);
    }

    /// 加工成功時に取り消し用の元テキストを記録する
    ///
    /// # Arguments
    /// * `text` - 取り消し用に保持する加工前テキスト
    pub fn record_undo_source(&self, text: &str) {
        self.clipboard_session.record_undo_source(text);
    }

    /// 取り消し用の元テキストを取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 取り消し可能な加工があれば `Some(元テキスト)`
    pub fn take_undo_source(&self) -> Option<SecretString> {
        self.clipboard_session.take_undo_source()
    }

    /// 履歴の件数を返す
    ///
    /// # Returns
    /// * `usize` - 保持している履歴件数
    pub fn history_len(&self) -> usize {
        self.clipboard_session.history_len()
    }

    /// 指定インデックスの履歴を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 履歴ストア内のインデックス (0 が最新)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_history_entry(&self, index: usize) -> Option<SecretString> {
        self.clipboard_session.get_history_entry(index)
    }

    /// 履歴をクリアする
    pub fn clear_history(&self) {
        self.clipboard_session.clear_history();
    }

    /// クリップボード履歴に新しいテキストを追加する
    ///
    /// 追加完了後、UI スレッドへ履歴更新イベントを通知する
    ///
    /// # Arguments
    /// * `text` - 履歴へ追加するクリップボード本文
    pub fn add_to_history(&self, text: impl AsRef<str>) {
        let limit = self.with_config(|c| c.history_limit);
        if self.clipboard_session.add_to_history(text.as_ref(), limit) {
            dispatch::send_app_event(&self.proxy, AppEvent::RefreshHistory);
        }
    }

    /// 連続記録の件数を返す
    ///
    /// # Returns
    /// * `usize` - 保持している連続記録件数
    pub fn continuous_record_len(&self) -> usize {
        self.clipboard_session.continuous_record_len()
    }

    /// 指定インデックスの連続記録を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_continuous_record_entry(&self, index: usize) -> Option<SecretString> {
        self.clipboard_session.get_continuous_record_entry(index)
    }

    /// 連続記録をクリアする
    pub fn clear_continuous_record(&self) {
        self.clipboard_session.clear_continuous_record();
    }

    /// 連続記録に新しいテキストをキューへ積む
    ///
    /// 追加完了後、UI スレッドへ連続記録更新イベントを通知する
    ///
    /// # Arguments
    /// * `text` - 連続記録へ追加するクリップボード本文
    pub fn enqueue_continuous_record(&self, text: impl AsRef<str>) {
        let limit = self.with_config(|c| c.continuous_record_limit);
        if self
            .clipboard_session
            .enqueue_continuous_record(text.as_ref(), limit)
        {
            dispatch::send_app_event(&self.proxy, AppEvent::RefreshContinuousRecord);
        }
    }

    /// 連続記録キュー先頭を取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 先頭エントリ。空の場合は `None`
    pub fn discharge_continuous_record_front(&self) -> Option<SecretString> {
        let discharged = self.clipboard_session.discharge_continuous_record_front();
        if discharged.is_some() {
            dispatch::send_app_event(&self.proxy, AppEvent::RefreshContinuousRecord);
        }
        discharged
    }

    /// 連続記録キューから指定位置のエントリを取り出す
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 排出した本文。範囲外の場合は `None`
    pub fn discharge_continuous_record_at(&self, index: usize) -> Option<SecretString> {
        let discharged = self.clipboard_session.discharge_continuous_record_at(index);
        if discharged.is_some() {
            dispatch::send_app_event(&self.proxy, AppEvent::RefreshContinuousRecord);
        }
        discharged
    }
}

// ======================================================================
// テスト用ヘルパー
// ======================================================================
/// ユニットテスト用の `AppState` を生成する
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
#[allow(clippy::expect_used)]
pub(crate) fn test_app_state() -> AppState {
    use crate::config::AppConfig;
    use crate::refiner::RefineMode;

    use tao::event_loop::EventLoopBuilder;
    use tao::platform::windows::EventLoopBuilderExtWindows;

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();

    let config = AppConfig {
        mode: RefineMode::Trim,
        ..Default::default()
    };

    AppState {
        worker: WorkerState::new_for_test(),
        config: RwLock::new(config),
        monitor_generation: AtomicU64::new(0),
        clipboard_session: ClipboardSessionState::new()
            .expect("テスト用クリップボードセッションの生成に失敗"),
        config_sync: ConfigSyncState::new_for_test(),
        settings_pending: SettingsPendingState::new(),
        proxy: event_loop.create_proxy(),
    }
}
