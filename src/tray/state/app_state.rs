use std::sync::{Mutex, RwLock, atomic::AtomicU64};
use std::time::{Duration, Instant, SystemTime};

use super::super::dispatch;
use super::super::history::EncryptedHistoryStore;
use super::app_event::AppEvent;
use super::lock_ext::{LockExt, RwLockExt};
use super::monitor_snapshot::{MonitorSnapshot, ProcessedState};

use crate::config::AppConfig;
use crate::security::{SecretString, secret_from};

use tao::event_loop::EventLoopProxy;

// ======================================================================
// アプリケーション状態
// ======================================================================
/// アプリケーション全体で共有され、スレッド間で安全に読み書きされる状態管理構造体
///
/// 設定、クリップボードの最終処理内容、暗号化履歴などを保持する
pub struct AppState {
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
    /// メインのイベントループへメッセージを送るためのプロキシ
    pub proxy: EventLoopProxy<AppEvent>,
    /// 設定をディスクへ保存するかどうか
    persist_config: bool,
    /// ディスク上の設定ファイルと同期済みの最終更新時刻
    config_disk_mtime: Mutex<Option<SystemTime>>,
    /// 自身の保存直後に外部変更検知を抑制する期限
    config_save_grace_until: Mutex<Option<Instant>>,
}

// ======================================================================
// 初期化・設定保存
// ======================================================================
impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Returns
    /// * `Result<Self>` - 新しく生成された `AppState` インスタンス
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let config = AppConfig::load();
        let disk_mtime = crate::config::disk_config_modified_time().ok().flatten();
        Ok(Self {
            config: RwLock::new(config),
            monitor_generation: AtomicU64::new(0),
            processed_state: Mutex::new(ProcessedState::default()),
            undo_text: Mutex::new(None),
            history_store: Mutex::new(EncryptedHistoryStore::new()?),
            proxy,
            persist_config: true,
            config_disk_mtime: Mutex::new(disk_mtime),
            config_save_grace_until: Mutex::new(None),
        })
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
            ps.last_written = None;
            ps.last_seen = crate::security::ContentFingerprint::from_text(text);
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
    #[cfg(windows)]
    use tao::platform::windows::EventLoopBuilderExtWindows;

    #[cfg(windows)]
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    #[cfg(not(windows))]
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

    let config = AppConfig {
        mode: RefineMode::Trim,
        ..Default::default()
    };

    AppState {
        config: RwLock::new(config),
        monitor_generation: AtomicU64::new(0),
        processed_state: Mutex::new(ProcessedState::default()),
        undo_text: Mutex::new(None),
        history_store: Mutex::new(
            EncryptedHistoryStore::new().expect("テスト用履歴ストアの生成に失敗"),
        ),
        proxy: event_loop.create_proxy(),
        persist_config: false,
        config_disk_mtime: Mutex::new(None),
        config_save_grace_until: Mutex::new(None),
    }
}
