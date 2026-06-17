use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::clipboard_change::ChangeWatcher;
use super::monitor::{self, EVENT_POLL_MS, POLL_TICK_MS};
use super::notifier;
use super::state::AppState;
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

use arboard::Clipboard;

// ======================================================================
// コマンド定義
// ======================================================================
/// UI・監視ループからクリップボードワーカーへ送られる操作コマンド
#[derive(Clone)]
pub enum ClipboardCommand {
    /// 指定されたテキストをクリップボードにセットする（履歴からの復元用など）
    SetText(String),
    /// 現在のクリップボード内容を指定されたモードで加工する
    ProcessMode(RefineMode),
}

impl std::fmt::Debug for ClipboardCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetText(_) => f.debug_tuple("SetText").field(&"...").finish(),
            Self::ProcessMode(mode) => f.debug_tuple("ProcessMode").field(mode).finish(),
        }
    }
}

// ======================================================================
// 監視ループ状態
// ======================================================================
/// ワーカースレッド内でクリップボード監視を行うための状態
///
/// 監視世代、ポーリング間隔、イベントトークンの追跡を担当します。
struct MonitorLoopState {
    /// 現在追跡中の監視世代（0 は監視停止中）
    tracked_generation: u64,
    /// 前回ポーリング監視を実行した時刻
    last_poll_at: Instant,
    /// イベント監視用の前回トークン
    last_token: u64,
    /// イベント監視フォールバック警告を既に出力したか
    event_fallback_warned: bool,
}

impl MonitorLoopState {
    /// 新しい監視ループ状態を生成する
    ///
    /// # Returns
    /// * `Self` - 監視停止状態の初期インスタンス。
    fn new() -> Self {
        Self {
            tracked_generation: 0,
            last_poll_at: Instant::now(),
            last_token: 0,
            event_fallback_warned: false,
        }
    }

    /// 監視世代の切り替え時にベースラインを再同期する
    ///
    /// 現在のクリップボード本文を観測済みとして記録し、イベント監視用トークンと
    /// ポーリング用タイマーをリセットします。
    ///
    /// # Arguments
    /// * `clipboard` - クリップボード操作用のインスタンス
    /// * `state` - アプリケーションの共有状態
    /// * `watcher` - クリップボード変更検知用ウォッチャー
    fn reset(&mut self, clipboard: &mut Clipboard, state: &Arc<AppState>, watcher: &ChangeWatcher) {
        let text = clipboard.get_text().unwrap_or_default();
        state.record_clipboard_observed(&text);
        self.last_token = watcher.token().unwrap_or(0);
        self.last_poll_at = Instant::now();
        self.tracked_generation = state.monitor_generation.load(Ordering::SeqCst);
        self.event_fallback_warned = false;
    }

    /// 設定と環境に応じた実効監視モードを返す
    ///
    /// イベント監視が利用できない場合はポーリング方式にフォールバックします。
    ///
    /// # Arguments
    /// * `watcher` - クリップボード変更検知用ウォッチャー
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `MonitorMode` - 実際に使用する監視方式。
    fn effective_mode(&self, watcher: &ChangeWatcher, state: &Arc<AppState>) -> MonitorMode {
        match state.with_config(|c| c.monitor_mode) {
            MonitorMode::Event if watcher.is_supported() => MonitorMode::Event,
            MonitorMode::Event => MonitorMode::Polling,
            MonitorMode::Polling => MonitorMode::Polling,
        }
    }

    /// コマンド待ちのタイムアウトを計算する
    ///
    /// 監視が停止中の場合は短い間隔で再確認し、
    /// 監視中はポーリング間隔またはイベント監視間隔に応じた値を返します。
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    /// * `watcher` - クリップボード変更検知用ウォッチャー
    ///
    /// # Returns
    /// * `Duration` - `recv_timeout` に渡す待機時間。
    fn recv_timeout(&self, state: &Arc<AppState>, watcher: &ChangeWatcher) -> Duration {
        let generation = state.monitor_generation.load(Ordering::SeqCst);
        if state.with_config(|c| c.is_paused)
            || generation == 0
            || self.tracked_generation != generation
        {
            return Duration::from_millis(100);
        }

        match self.effective_mode(watcher, state) {
            MonitorMode::Polling => {
                let snap = state.monitor_snapshot();
                let elapsed = self.last_poll_at.elapsed().as_millis() as u64;
                let remaining = snap.interval_ms.saturating_sub(elapsed);
                Duration::from_millis(POLL_TICK_MS.min(remaining.max(1)))
            }
            MonitorMode::Event => Duration::from_millis(POLL_TICK_MS.min(EVENT_POLL_MS)),
        }
    }

    /// 監視ループの1ティックを実行する
    ///
    /// 実効監視モードに応じてクリップボードの変更を検知し、
    /// 必要に応じて加工処理を委譲します。
    ///
    /// # Arguments
    /// * `clipboard` - クリップボード操作用のインスタンス
    /// * `state` - アプリケーションの共有状態
    /// * `watcher` - クリップボード変更検知用ウォッチャー
    fn tick(&mut self, clipboard: &mut Clipboard, state: &Arc<AppState>, watcher: &ChangeWatcher) {
        let snap = state.monitor_snapshot();
        if snap.is_paused {
            return;
        }

        if state.with_config(|c| c.monitor_mode) == MonitorMode::Event
            && !watcher.is_supported()
            && !self.event_fallback_warned
        {
            crate::log_warn!(
                "イベント監視が利用できないため、ポーリング監視にフォールバックします"
            );
            self.event_fallback_warned = true;
        }

        match self.effective_mode(watcher, state) {
            MonitorMode::Event => {
                if let Some(token) = watcher.token()
                    && token != self.last_token
                {
                    self.last_token = token;
                    if monitor::handle_clipboard_update(clipboard, state, &snap, true) {
                        self.last_token = watcher.token().unwrap_or(self.last_token);
                    }
                }
            }
            MonitorMode::Polling => {
                if self.last_poll_at.elapsed() >= Duration::from_millis(snap.interval_ms) {
                    monitor::handle_clipboard_update(clipboard, state, &snap, false);
                    self.last_poll_at = Instant::now();
                }
            }
        }
    }
}

// ======================================================================
// ワーカースレッド
// ======================================================================
/// クリップボード操作と監視を単一スレッドで処理するワーカーを開始する
///
/// すべてのクリップボード読み書きはこのスレッドに集約され、
/// UI からのコマンドと監視ループが `recv_timeout` で交互に処理されます。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `Sender<ClipboardCommand>` - ワーカーに操作を依頼するためのチャネル送信端
pub fn spawn_clipboard_worker(state: Arc<AppState>) -> Sender<ClipboardCommand> {
    let (tx, rx): (Sender<ClipboardCommand>, Receiver<ClipboardCommand>) = mpsc::channel();

    thread::spawn(move || run_worker_loop(state, rx));

    tx
}

/// クリップボードワーカースレッドのメインループを実行する
///
/// クリップボードの初期化、変更検知ウォッチャーの生成、監視ループ状態の管理を行い、
/// コマンド受信と監視処理を交互に実行します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `rx` - ワーカーに操作を依頼するためのチャネル受信端
fn run_worker_loop(state: Arc<AppState>, rx: Receiver<ClipboardCommand>) {
    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            crate::log_error!("クリップボード初期化エラー: {:?}", e);
            notification::show_notification(
                "クリップボードエラー",
                "クリップボードの初期化に失敗しました。監視処理は停止します。",
            );
            return;
        }
    };

    let watcher = ChangeWatcher::new();
    let mut monitor = MonitorLoopState::new();

    loop {
        sync_monitor_generation(&mut monitor, &mut clipboard, &state, &watcher);

        let timeout = monitor.recv_timeout(&state, &watcher);
        match rx.recv_timeout(timeout) {
            Ok(cmd) => handle_command(&mut clipboard, &state, cmd),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if should_run_monitor_tick(&state, &monitor) {
            monitor.tick(&mut clipboard, &state, &watcher);
        }
    }
}

/// 監視ループの1ティックを実行するかどうかを判定する
///
/// 監視が停止中の場合はfalseを返し、
/// 監視中は監視世代が一致している場合はtrueを返します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `monitor` - 監視ループ状態
///
/// # Returns
/// * `bool` - 監視ループの1ティックを実行するかどうか。
fn should_run_monitor_tick(state: &Arc<AppState>, monitor: &MonitorLoopState) -> bool {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    !state.with_config(|c| c.is_paused)
        && generation > 0
        && monitor.tracked_generation == generation
}

/// 監視世代を同期する
///
/// 現在のクリップボード本文を観測済みとして記録し、イベント監視用トークンと
/// ポーリング用タイマーをリセットします。
///
/// # Arguments
/// * `monitor` - 監視ループ状態
/// * `clipboard` - クリップボード操作用のインスタンス
/// * `state` - アプリケーションの共有状態
/// * `watcher` - クリップボード変更検知用ウォッチャー
fn sync_monitor_generation(
    monitor: &mut MonitorLoopState,
    clipboard: &mut Clipboard,
    state: &Arc<AppState>,
    watcher: &ChangeWatcher,
) {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    let paused = state.with_config(|c| c.is_paused);

    if paused || generation == 0 {
        monitor.tracked_generation = 0;
        return;
    }

    if monitor.tracked_generation != generation {
        monitor.reset(clipboard, state, watcher);
    }
}

/// コマンドを処理する
///
/// クリップボードの設定や加工を行い、成功時に通知を表示します。
///
/// # Arguments
/// * `clipboard` - クリップボード操作用のインスタンス
/// * `state` - アプリケーションの共有状態
/// * `cmd` - 受信したコマンド
fn handle_command(clipboard: &mut Clipboard, state: &Arc<AppState>, cmd: ClipboardCommand) {
    match cmd {
        ClipboardCommand::SetText(text) => {
            if let Err(e) = clipboard.set_text(text.clone()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                notification::show_notification(
                    "クリップボードエラー",
                    "履歴からの復元処理に失敗しました。",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    notification::show_notification(
                        "履歴から復元",
                        "クリップボードにコピーしました",
                    );
                }
            }
        }
        ClipboardCommand::ProcessMode(mode) => {
            if let Some(processed) = process_clipboard(clipboard, mode) {
                state.record_processing_success(&processed);
                notifier::show_process_notification(state, mode, &processed);
            }
        }
    }
}
