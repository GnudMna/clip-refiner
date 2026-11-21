use std::sync::{RwLock, RwLockWriteGuard, atomic::AtomicU64};

use tao::event_loop::EventLoopProxy;

use super::super::dispatch;
use super::app_event::{AppEvent, MenuRefreshEvent};
use super::clipboard_session::ClipboardSessionState;
use super::config_sync::ConfigSyncState;
use super::lock_ext::RwLockExt;
use super::menu_snapshot::MenuSnapshot;
use super::monitor_snapshot::MonitorSnapshot;
use super::settings_pending::SettingsPendingState;
use super::worker_state::WorkerState;
use crate::config::{AppConfig, ConfigLoadFallback, RegexSettings};
use crate::security::SecretString;

// ======================================================================
// アプリケーション状態
// ======================================================================
/// アプリケーション全体で共有され、スレッド間で安全に読み書きされる状態管理構造体
///
/// 永続設定・ワーカー稼働・監視世代・セッション揮発データ・設定ウィンドウ適用待ちを保持する
/// 単一サブ状態で完結する操作は各アクセサ経由で呼び出し、複数フィールドにまたがる処理のみここに置く
pub struct AppState {
    worker: WorkerState,
    /// 永続化設定データ
    ///
    /// 書き込みは `with_config_mut` に限定し、パニック時のロールバックを必ず通す
    config: RwLock<AppConfig>,
    /// クリップボード監視スレッドの世代管理カウンタ
    pub monitor_generation: AtomicU64,
    clipboard_session: ClipboardSessionState,
    config_sync: ConfigSyncState,
    settings_pending: SettingsPendingState,
    /// メインのイベントループへメッセージを送るためのプロキシ
    pub proxy: EventLoopProxy<AppEvent>,
}

// ======================================================================
// 初期化・設定保存・アクセサ
// ======================================================================
impl AppState {
    /// 設定ファイルを読み込んで新しい状態を生成する
    ///
    /// # Arguments
    /// * `proxy` - メインイベントループへ通知を送るプロキシ
    ///
    /// # Returns
    /// * `Result<(Self, Option<ConfigLoadFallback>, Vec<String>, bool)>` - 生成された `AppState`、
    ///   設定読み込み失敗時のフォールバック理由、認識できない設定キー、
    ///   未対応スキーマ version を既存項目維持で読み込んだか
    pub fn new(
        proxy: EventLoopProxy<AppEvent>,
    ) -> anyhow::Result<(Self, Option<ConfigLoadFallback>, Vec<String>, bool)> {
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
            load_outcome.unknown_keys,
            load_outcome.unsupported_version_kept,
        ))
    }

    /// クリップボードワーカーの稼働状態
    pub(crate) fn worker(&self) -> &WorkerState {
        &self.worker
    }

    /// セッション限定の監視・履歴・連続記録状態
    pub(crate) fn clipboard_session(&self) -> &ClipboardSessionState {
        &self.clipboard_session
    }

    /// 設定ファイルのディスク同期状態
    pub(crate) fn config_sync(&self) -> &ConfigSyncState {
        &self.config_sync
    }

    /// 設定ウィンドウからの適用待ち要求
    pub(crate) fn settings_pending(&self) -> &SettingsPendingState {
        &self.settings_pending
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// テスト用 `AppState` ではディスクへ書き込まない
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn save_config(&self) {
        if !self.config_sync.should_persist() {
            return;
        }
        self.config_sync.begin_own_write();
        let save_result = self.with_config(crate::config::AppConfig::save);
        match save_result {
            Ok(()) => self.config_sync.record_disk_sync(),
            Err(e) => crate::user_error::config_save_failure().notify_with_debug(&e),
        }
        self.config_sync.end_own_write();
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
    /// クロージャが変更の途中でパニックした場合は、呼び出し前の内容へ戻してからロックを解放する
    /// ポイズン解除後に中途半端な設定を正常なものとして使い続け、次の保存でディスクへ書くのを防ぐ
    ///
    /// # Arguments
    /// * `f` - `AppConfig` へ排他アクセスして変更を行うクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャ `f` の戻り値
    pub fn with_config_mut<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> R {
        let guard = self.config.write_ignore_poison();
        let mut rollback = ConfigRollback {
            snapshot: Some(guard.clone()),
            guard,
        };
        let result = f(&mut rollback.guard);
        rollback.snapshot = None;
        result
    }
}

/// `with_config_mut` のクロージャがパニックしたときに設定を変更前へ戻すガード
///
/// 書き込みロックより先に `Drop` が走るよう、ロックガードを内包する
struct ConfigRollback<'a> {
    guard: RwLockWriteGuard<'a, AppConfig>,
    /// 変更前の設定。正常終了時は `None` にして復元しない
    snapshot: Option<AppConfig>,
}

impl Drop for ConfigRollback<'_> {
    fn drop(&mut self) {
        if let Some(snapshot) = self.snapshot.take() {
            crate::log_error!("設定の変更中にパニックが発生したため、変更前の設定へ戻します");
            *self.guard = snapshot;
        }
    }
}

// ======================================================================
// 横断操作 (設定・セッション・イベント連携)
// ======================================================================
impl AppState {
    /// 監視ループ向けに設定フィールドをまとめて一括取得する
    ///
    /// `config` `RwLock` の取得を1回に抑えることで、ループ毎の細粒度ロックを削減する
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `MonitorSnapshot` - 監視ループ向けの設定フィールドの写し
    pub fn monitor_snapshot(&self) -> MonitorSnapshot {
        self.with_config(|config| MonitorSnapshot {
            pipeline: config.effective_pipeline(),
            interval_ms: config.interval_ms,
            auto_refine_enabled: config.auto_refine_enabled,
            history_enabled: config.history_enabled,
            continuous_record_enabled: config.continuous_record_enabled,
            monitor_mode: config.monitor_mode,
        })
    }

    /// 加工用の正規表現設定を現在の設定へ同期する
    ///
    /// 内容が異なる場合のみ複製するため、変更のないティックではコピーが発生しない
    ///
    /// # Arguments
    /// * `dst` - 同期先の正規表現設定
    pub fn sync_regex_settings(&self, dst: &mut RegexSettings) {
        self.with_config(|config| {
            if config.regex != *dst {
                dst.clone_from(&config.regex);
            }
        });
    }

    /// トレイメニュー構築・同期向けに設定フィールドを一括取得する
    ///
    /// 登録クリップ一覧を含む `AppConfig` 全体の clone を避け、メニュー表示に必要な
    /// フィールドだけをコピーする
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `MenuSnapshot` - トレイメニュー構築・同期向けの設定フィールドの写し
    pub fn menu_snapshot(&self) -> MenuSnapshot {
        self.with_config(|config| MenuSnapshot {
            mode: config.mode,
            active_modes: config.effective_pipeline().into_iter().collect(),
            favorite_modes: config.favorite_modes.clone(),
            hotkeys: config.hotkeys.clone(),
            history_enabled: config.history_enabled,
            continuous_record_enabled: config.continuous_record_enabled,
            regex_active_preset_id: config.regex.active_preset_id.clone(),
            regex_presets: config
                .regex
                .presets
                .iter()
                .map(|preset| (preset.id.clone(), preset.name.clone()))
                .collect(),
        })
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
            dispatch::send_app_event(&self.proxy, MenuRefreshEvent::History.into());
        }
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
            dispatch::send_app_event(&self.proxy, MenuRefreshEvent::ContinuousRecord.into());
        }
    }

    /// 連続記録キュー先頭を取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<SecretString>` - 値がある場合は `Some`、ない場合は `None`
    pub fn discharge_continuous_record_front(&self) -> Option<SecretString> {
        let discharged = self.clipboard_session.discharge_continuous_record_front();
        if discharged.is_some() {
            dispatch::send_app_event(&self.proxy, MenuRefreshEvent::ContinuousRecord.into());
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
            dispatch::send_app_event(&self.proxy, MenuRefreshEvent::ContinuousRecord.into());
        }
        discharged
    }

    /// 取り出した連続記録を元のインデックスへ戻す
    ///
    /// クリップボードへの書き込みに失敗した場合に使う。戻した後、UI スレッドへ連続記録更新イベントを通知する
    ///
    /// # Arguments
    /// * `index` - 戻す位置 (0 が先頭)
    /// * `text` - 戻すクリップボード本文
    pub fn restore_continuous_record_at(&self, index: usize, text: impl AsRef<str>) {
        let limit = self.with_config(|c| c.continuous_record_limit);
        if self
            .clipboard_session
            .restore_continuous_record_at(index, text.as_ref(), limit)
        {
            dispatch::send_app_event(&self.proxy, MenuRefreshEvent::ContinuousRecord.into());
        }
    }

    /// 連続記録キュー末尾 (直近の記録) を取り消す
    ///
    /// 取り消し成功時は UI スレッドへ連続記録更新イベントを通知する
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - 取り消した場合は `true`、記録が空の場合は `false`
    pub fn undo_last_continuous_record(&self) -> bool {
        let undone = self.clipboard_session.undo_last_continuous_record();
        if undone.is_some() {
            dispatch::send_app_event(&self.proxy, MenuRefreshEvent::ContinuousRecord.into());
            return true;
        }
        false
    }
}

// ======================================================================
// テスト用ヘルパー
// ======================================================================
/// ユニットテスト用の `AppState` を生成する
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn test_app_state() -> AppState {
    test_app_state_inner(false)
}

/// ディスク永続化を有効にしたテスト用 `AppState` を生成する
///
/// `with_temp_config_dir` 内でのみ使うこと
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn test_app_state_with_persistence() -> AppState {
    test_app_state_inner(true)
}

#[allow(
    clippy::expect_used,
    reason = "テスト専用のため、生成失敗はそのまま panic させる"
)]
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
fn test_app_state_inner(persist_config: bool) -> AppState {
    use tao::event_loop::EventLoopBuilder;
    use tao::platform::windows::EventLoopBuilderExtWindows;

    use crate::config::AppConfig;
    use crate::refiner::RefineMode;

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();

    // 監視経路を検証するテストが多いため、新規既定と異なり自動変換を有効にする
    let config = AppConfig {
        auto_refine_enabled: true,
        mode: RefineMode::Trim,
        ..Default::default()
    };

    AppState {
        worker: WorkerState::new_for_test(),
        config: RwLock::new(config),
        monitor_generation: AtomicU64::new(0),
        clipboard_session: ClipboardSessionState::new()
            .expect("テスト用クリップボードセッションの生成に失敗"),
        config_sync: if persist_config {
            ConfigSyncState::new_for_test_with_persistence()
        } else {
            ConfigSyncState::new_for_test()
        },
        settings_pending: SettingsPendingState::new(),
        proxy: event_loop.create_proxy(),
    }
}
