use std::sync::{
    Mutex, RwLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime};

use super::super::dispatch;
use super::super::history::EncryptedHistoryStore;
use super::super::history::EncryptedRecordStore;
use super::app_event::AppEvent;
use super::lock_ext::{LockExt, RwLockExt};
use super::monitor_snapshot::{MonitorSnapshot, ProcessedState};

use crate::config::{AppConfig, ConfigLoadFallback, RegexSettings};
use crate::security::{SecretString, secret_from};

use tao::event_loop::EventLoopProxy;

// ======================================================================
// 正規表現プレビュー要求
// ======================================================================
/// 設定ウィンドウからの正規表現プレビュー要求
#[derive(Debug, Clone)]
pub(crate) struct RegexPreviewRequest {
    /// クライアント側リクエスト ID (応答の突合に使用)
    pub request_id: u64,
    /// プレビュー対象の正規表現設定
    pub settings: RegexSettings,
}

// ======================================================================
// アプリケーション状態
// ======================================================================
/// アプリケーション全体で共有され、スレッド間で安全に読み書きされる状態管理構造体
///
/// 設定、クリップボードの最終処理内容、暗号化履歴などを保持する
pub struct AppState {
    /// クリップボードワーカーが稼働中かどうか
    worker_alive: AtomicBool,
    /// ワーカー再起動成功時にユーザーへ通知するかどうか
    worker_recovery_pending: AtomicBool,
    /// 永続化設定データ
    pub config: RwLock<AppConfig>,
    /// クリップボード監視スレッドの世代管理カウンタ
    pub monitor_generation: AtomicU64,
    /// 二重加工防止用の監視状態
    processed_state: Mutex<ProcessedState>,
    /// 直近の加工前テキスト (取り消し用、破棄時にゼロ化)
    undo_text: Mutex<Option<SecretString>>,
    /// 暗号化されたクリップボード履歴 (セッション限定、メモリ内のみ)
    history_store: Mutex<EncryptedHistoryStore>,
    /// 暗号化された連続記録 (セッション限定、メモリ内のみ)
    continuous_record_store: Mutex<EncryptedRecordStore>,
    /// メインのイベントループへメッセージを送るためのプロキシ
    pub proxy: EventLoopProxy<AppEvent>,
    /// 設定をディスクへ保存するかどうか
    persist_config: bool,
    /// ディスク上の設定ファイルと同期済みの最終更新時刻
    config_disk_mtime: Mutex<Option<SystemTime>>,
    /// 自身の保存直後に外部変更検知を抑制する期限
    config_save_grace_until: Mutex<Option<Instant>>,
    /// 設定ウィンドウから受信した部分更新 JSON (適用待ち)
    pending_settings_patch: Mutex<Option<String>>,
    /// 設定ウィンドウからの正規表現プレビュー要求 (処理待ち)
    pending_regex_preview: Mutex<Option<RegexPreviewRequest>>,
}

// ======================================================================
// 初期化・設定保存
// ======================================================================
impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Returns
    /// * `Result<(Self, Option<ConfigLoadFallback>)>` - 生成された `AppState` と、設定読み込み失敗時のフォールバック理由
    pub fn new(
        proxy: EventLoopProxy<AppEvent>,
    ) -> anyhow::Result<(Self, Option<ConfigLoadFallback>)> {
        let load_outcome = AppConfig::load();
        let config = load_outcome.config;
        let disk_mtime = crate::config::disk_config_modified_time().ok().flatten();
        Ok((
            Self {
                config: RwLock::new(config),
                worker_alive: AtomicBool::new(false),
                worker_recovery_pending: AtomicBool::new(false),
                monitor_generation: AtomicU64::new(0),
                processed_state: Mutex::new(ProcessedState::default()),
                undo_text: Mutex::new(None),
                history_store: Mutex::new(EncryptedHistoryStore::new()?),
                continuous_record_store: Mutex::new(EncryptedRecordStore::new()?),
                proxy,
                persist_config: true,
                config_disk_mtime: Mutex::new(disk_mtime),
                config_save_grace_until: Mutex::new(None),
                pending_settings_patch: Mutex::new(None),
                pending_regex_preview: Mutex::new(None),
            },
            load_outcome.fallback,
        ))
    }

    /// 適用待ちの部分更新 JSON キューへ排他アクセスする
    pub(crate) fn pending_settings_patch_mut(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.pending_settings_patch.lock_ignore_poison()
    }

    /// キュー済みの部分更新 JSON を取り出す
    pub(crate) fn take_pending_settings_patch(&self) -> Option<String> {
        self.pending_settings_patch.lock_ignore_poison().take()
    }

    /// 正規表現プレビュー要求を格納する
    pub(crate) fn set_regex_preview_request(&self, request: RegexPreviewRequest) {
        *self.pending_regex_preview.lock_ignore_poison() = Some(request);
    }

    /// 処理待ちの正規表現プレビュー要求を取り出す
    pub(crate) fn take_regex_preview_request(&self) -> Option<RegexPreviewRequest> {
        self.pending_regex_preview.lock_ignore_poison().take()
    }

    /// ディスク上の設定ファイルと同期済みの更新時刻を記録する
    pub fn record_config_disk_sync(&self) {
        if let Ok(mtime) = crate::config::disk_config_modified_time() {
            *self.config_disk_mtime.lock_ignore_poison() = mtime;
        }
    }

    /// 外部編集による設定ファイルの変更を検知する
    ///
    /// アプリ自身の保存直後は同期済み時刻と一致するため `false` を返す
    pub fn has_external_config_change(&self) -> bool {
        if self.is_config_save_grace_active() {
            return false;
        }
        let Ok(Some(file_mtime)) = crate::config::disk_config_modified_time() else {
            return false;
        };
        match *self.config_disk_mtime.lock_ignore_poison() {
            Some(known) => file_mtime > known,
            None => true,
        }
    }

    /// 自身の保存直後のグレース期間中かどうか
    fn is_config_save_grace_active(&self) -> bool {
        self.config_save_grace_until
            .lock_ignore_poison()
            .is_some_and(|until| Instant::now() < until)
    }

    /// 自身の保存直後に外部変更検知を一時抑制する
    fn begin_config_save_grace(&self) {
        *self.config_save_grace_until.lock_ignore_poison() =
            Some(Instant::now() + Duration::from_secs(2));
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// `persist_config` が `false` の場合はメモリ上の変更のみとし、ディスクへは書き込まない
    pub fn save_config(&self) {
        if !self.persist_config {
            return;
        }
        if let Err(e) = self.with_config(crate::config::AppConfig::save) {
            crate::log_error!("設定の保存に失敗: {:?}", e);
            return;
        }
        self.begin_config_save_grace();
        self.record_config_disk_sync();
    }

    /// 設定を読み取り専用で参照する
    pub fn with_config<R>(&self, f: impl FnOnce(&AppConfig) -> R) -> R {
        f(&self.config.read_ignore_poison())
    }

    /// 設定を変更する
    pub fn with_config_mut<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> R {
        f(&mut self.config.write_ignore_poison())
    }

    /// クリップボードワーカーが稼働中かどうか
    pub fn is_worker_alive(&self) -> bool {
        self.worker_alive.load(Ordering::SeqCst)
    }

    /// クリップボードワーカーの稼働状態を更新する
    pub fn set_worker_alive(&self, alive: bool) {
        self.worker_alive.store(alive, Ordering::SeqCst);
    }

    /// ワーカー復旧成功時の通知が必要であることを記録する
    pub fn set_worker_recovery_pending(&self, pending: bool) {
        self.worker_recovery_pending
            .store(pending, Ordering::SeqCst);
    }

    /// ワーカー復旧成功時の通知が必要かどうかを取得し、フラグをクリアする
    pub fn take_worker_recovery_pending(&self) -> bool {
        self.worker_recovery_pending.swap(false, Ordering::SeqCst)
    }

    /// 設定のディスク保存が有効かどうかを返す
    #[cfg(test)]
    pub(crate) fn is_config_persistence_enabled(&self) -> bool {
        self.persist_config
    }
}

// ======================================================================
// 状態操作
// ======================================================================
impl AppState {
    /// 監視ループ向けに設定フィールドをまとめて一括取得する
    ///
    /// `config` `RwLock` の取得を1回に抑えることで、ループ毎の細粒度ロックを削減する
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
    pub fn with_processed_state<R>(&self, f: impl FnOnce(&mut ProcessedState) -> R) -> R {
        f(&mut self.processed_state.lock_ignore_poison())
    }

    /// 加工成功後にクリップボードへ書き戻したことを記録する
    pub fn record_processing_success(&self, output: &str) {
        self.with_processed_state(|ps| {
            let fp = crate::security::ContentFingerprint::from_text(output);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 画像加工成功後に元テキストの指紋を記録する
    ///
    /// クリップボード上に TSV が残る場合の再加工ループを防ぐ
    pub fn record_image_processing_success(&self, source_text: &str) {
        self.with_processed_state(|ps| {
            let fp = crate::security::ContentFingerprint::from_text(source_text);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 加工せずに観測したクリップボード本文を記録する
    pub fn record_clipboard_observed(&self, text: &str) {
        self.with_processed_state(|ps| {
            ps.last_seen = crate::security::ContentFingerprint::from_text(text);
        });
    }

    /// 履歴復元など、外部からクリップボードへ設定した本文を記録する
    pub fn record_clipboard_set(&self, text: &str) {
        self.with_processed_state(|ps| {
            let fp = crate::security::ContentFingerprint::from_text(text);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 加工成功時に取り消し用の元テキストを記録する
    pub fn record_undo_source(&self, text: &str) {
        *self.undo_text.lock_ignore_poison() = Some(secret_from(text));
    }

    /// 取り消し用の元テキストを取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 取り消し可能な加工があれば `Some(元テキスト)`
    pub fn take_undo_source(&self) -> Option<SecretString> {
        self.undo_text.lock_ignore_poison().take()
    }

    /// 履歴の件数を返す
    pub fn history_len(&self) -> usize {
        self.history_store.lock_ignore_poison().len()
    }

    /// 指定インデックスの履歴を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 履歴ストア内のインデックス (0 が最新)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_history_entry(&self, index: usize) -> Option<SecretString> {
        self.history_store.lock_ignore_poison().entry_at(index)
    }

    /// 履歴をクリアする
    pub fn clear_history(&self) {
        self.history_store.lock_ignore_poison().clear();
    }

    /// クリップボード履歴に新しいテキストを追加する
    ///
    /// 空文字やトリム後に空になる文字列は無視される
    /// 既にある文字列はリストの先頭に移動し、設定の `history_limit` を超えた分は削除される
    /// 本文はメモリ上で暗号化して保持し、再起動時には破棄される
    /// 追加完了後、UIスレッドへ履歴更新イベントを通知する
    ///
    /// # Arguments
    /// * `text` - 履歴へ追加するクリップボード本文
    pub fn add_to_history(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.trim().is_empty() {
            return;
        }

        let limit = self.with_config(|c| c.history_limit);
        self.history_store.lock_ignore_poison().add(text, limit);

        dispatch::send_app_event(&self.proxy, AppEvent::RefreshHistory);
    }

    /// 連続記録の件数を返す
    pub fn continuous_record_len(&self) -> usize {
        self.continuous_record_store.lock_ignore_poison().len()
    }

    /// 指定インデックスの連続記録を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_continuous_record_entry(&self, index: usize) -> Option<SecretString> {
        self.continuous_record_store
            .lock_ignore_poison()
            .entry_at(index)
    }

    /// 連続記録をクリアする
    pub fn clear_continuous_record(&self) {
        self.continuous_record_store.lock_ignore_poison().clear();
    }

    /// 連続記録に新しいテキストをキューへ積む
    ///
    /// 空文字やトリム後に空になる文字列は無視される
    /// 同一内容でも末尾へ追加し、上限超過時は先頭から破棄する
    /// 追加完了後、UIスレッドへ連続記録更新イベントを通知する
    pub fn enqueue_continuous_record(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.trim().is_empty() {
            return;
        }

        let limit = self.with_config(|c| c.continuous_record_limit);
        self.continuous_record_store
            .lock_ignore_poison()
            .enqueue(text, limit);

        dispatch::send_app_event(&self.proxy, AppEvent::RefreshContinuousRecord);
    }

    /// 連続記録キュー先頭を取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 先頭エントリ。空の場合は `None`
    pub fn discharge_continuous_record_front(&self) -> Option<SecretString> {
        let discharged = self
            .continuous_record_store
            .lock_ignore_poison()
            .discharge_front();
        if discharged.is_some() {
            dispatch::send_app_event(&self.proxy, AppEvent::RefreshContinuousRecord);
        }
        discharged
    }

    /// 連続記録キューから指定位置のエントリを取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 排出した本文。範囲外の場合は `None`
    pub fn discharge_continuous_record_at(&self, index: usize) -> Option<SecretString> {
        let discharged = self
            .continuous_record_store
            .lock_ignore_poison()
            .discharge_at(index);
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
        config: RwLock::new(config),
        worker_alive: AtomicBool::new(true),
        worker_recovery_pending: AtomicBool::new(false),
        monitor_generation: AtomicU64::new(0),
        processed_state: Mutex::new(ProcessedState::default()),
        undo_text: Mutex::new(None),
        history_store: Mutex::new(
            EncryptedHistoryStore::new().expect("テスト用履歴ストアの生成に失敗"),
        ),
        continuous_record_store: Mutex::new(
            EncryptedRecordStore::new().expect("テスト用連続記録ストアの生成に失敗"),
        ),
        proxy: event_loop.create_proxy(),
        persist_config: false,
        config_disk_mtime: Mutex::new(None),
        config_save_grace_until: Mutex::new(None),
        pending_settings_patch: Mutex::new(None),
        pending_regex_preview: Mutex::new(None),
    }
}
