use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::clipboard_change::ChangeWatcher;
use super::clipboard_monitor::{self, EVENT_POLL_MS, POLL_TICK_MS};
use super::dispatch;
use super::notify;
use super::state::{AppEvent, AppState};

use crate::config::AddRegisteredTextError;
use crate::config::MonitorMode;
use crate::platform;
use crate::refiner::{
    ClipboardProcessOutcome, RefineContext, RefineMode, TextClipboard,
    process_clipboard_pipeline_io,
};
use crate::security::SecretString;

use arboard::Clipboard;

/// 設定ファイルの外部変更を検知するポーリング間隔
const CONFIG_POLL_INTERVAL: Duration = Duration::from_secs(2);

// ======================================================================
// コマンド定義
// ======================================================================
/// UI・監視ループからクリップボードワーカーへ送られる操作コマンド
#[derive(Clone)]
pub enum ClipboardCommand {
    /// 指定されたテキストをクリップボードにセットする(履歴からの復元用など)
    SetText(SecretString),
    /// 登録文字列をクリップボードにコピーする
    CopyRegisteredText(SecretString),
    /// 現在のクリップボード内容を指定されたモードで加工する
    ProcessMode(RefineMode),
    /// 直近の加工を取り消し、加工前のテキストをクリップボードへ復元する
    Undo,
    /// クリップボードの内容を登録文字列として保存する
    RegisterFromClipboard,
    /// OCR 結果をクリップボードへ書き込む
    SetOcrText(SecretString),
}

impl std::fmt::Debug for ClipboardCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetText(_) => f.debug_tuple("SetText").field(&"...").finish(),
            Self::CopyRegisteredText(_) => {
                f.debug_tuple("CopyRegisteredText").field(&"...").finish()
            }
            Self::ProcessMode(mode) => f.debug_tuple("ProcessMode").field(mode).finish(),
            Self::Undo => f.write_str("Undo"),
            Self::RegisterFromClipboard => f.write_str("RegisterFromClipboard"),
            Self::SetOcrText(_) => f.debug_tuple("SetOcrText").field(&"...").finish(),
        }
    }
}

// ======================================================================
// 監視ループ状態
// ======================================================================
/// ワーカースレッド内でクリップボード監視を行うための状態
///
/// 監視世代、ポーリング間隔、イベントトークンの追跡を担当する
struct MonitorLoopState {
    /// 現在追跡中の監視世代(0 は監視停止中)
    tracked_generation: u64,
    /// 前回ポーリング監視を実行した時刻
    last_poll_at: Instant,
    /// イベント監視用の前回トークン
    last_token: u64,
    /// イベント監視フォールバック警告を既に出力したか
    event_fallback_warned: bool,
    /// 加工コンテキスト (正規表現コンパイルキャッシュを保持)
    refine_ctx: RefineContext,
}

impl MonitorLoopState {
    /// 新しい監視ループ状態を生成する
    ///
    /// # Returns
    /// * `Self` - 監視停止状態の初期インスタンス
    fn new() -> Self {
        Self {
            tracked_generation: 0,
            last_poll_at: Instant::now(),
            last_token: 0,
            event_fallback_warned: false,
            refine_ctx: RefineContext::default(),
        }
    }

    /// 監視世代の切り替え時にベースラインを再同期する
    ///
    /// 現在のクリップボード本文を観測済みとして記録し、イベント監視用トークンと
    /// ポーリング用タイマーをリセットする
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
    /// イベント監視が利用できない場合はポーリング方式にフォールバックする
    ///
    /// # Arguments
    /// * `watcher` - クリップボード変更検知用ウォッチャー
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `MonitorMode` - 実際に使用する監視方式
    fn effective_mode(watcher: &ChangeWatcher, state: &Arc<AppState>) -> MonitorMode {
        match state.with_config(|c| c.monitor_mode) {
            MonitorMode::Event if watcher.is_supported() => MonitorMode::Event,
            MonitorMode::Event | MonitorMode::Polling => MonitorMode::Polling,
        }
    }

    /// コマンド待ちのタイムアウトを計算する
    ///
    /// 監視が停止中の場合は短い間隔で再確認し、
    /// 監視中はポーリング間隔またはイベント監視間隔に応じた値を返す
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    /// * `watcher` - クリップボード変更検知用ウォッチャー
    ///
    /// # Returns
    /// * `Duration` - `recv_timeout` に渡す待機時間
    fn recv_timeout(&self, state: &Arc<AppState>, watcher: &ChangeWatcher) -> Duration {
        let generation = state.monitor_generation.load(Ordering::SeqCst);
        if state.with_config(|c| c.is_paused)
            || generation == 0
            || self.tracked_generation != generation
        {
            return Duration::from_millis(100);
        }

        match Self::effective_mode(watcher, state) {
            MonitorMode::Polling => {
                let snap = state.monitor_snapshot();
                let elapsed =
                    u64::try_from(self.last_poll_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                let remaining = snap.interval_ms.saturating_sub(elapsed);
                Duration::from_millis(POLL_TICK_MS.min(remaining.max(1)))
            }
            MonitorMode::Event => Duration::from_millis(POLL_TICK_MS.min(EVENT_POLL_MS)),
        }
    }

    /// 監視ループの1ティックを実行する
    ///
    /// 実効監視モードに応じてクリップボードの変更を検知し、
    /// 必要に応じて加工処理を委譲する
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

        self.refine_ctx.regex = snap.regex_settings.clone();

        match Self::effective_mode(watcher, state) {
            MonitorMode::Event => {
                if let Some(token) = watcher.token()
                    && token != self.last_token
                {
                    self.last_token = token;
                    if clipboard_monitor::handle_clipboard_update(
                        clipboard,
                        state,
                        &snap,
                        true,
                        &self.refine_ctx,
                    ) {
                        self.last_token = watcher.token().unwrap_or(self.last_token);
                    }
                }
            }
            MonitorMode::Polling => {
                if self.last_poll_at.elapsed() >= Duration::from_millis(snap.interval_ms) {
                    clipboard_monitor::handle_clipboard_update(
                        clipboard,
                        state,
                        &snap,
                        false,
                        &self.refine_ctx,
                    );
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
/// UI からのコマンドと監視ループが `recv_timeout` で交互に処理される
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `Sender<ClipboardCommand>` - ワーカーに操作を依頼するためのチャネル送信端
pub fn spawn_clipboard_worker(state: Arc<AppState>) -> Sender<ClipboardCommand> {
    let (tx, rx): (Sender<ClipboardCommand>, Receiver<ClipboardCommand>) = mpsc::channel();

    thread::spawn(move || run_worker_loop(&state, &rx));

    tx
}

/// クリップボードワーカースレッドのメインループを実行する
///
/// クリップボードの初期化、変更検知ウォッチャーの生成、監視ループ状態の管理を行い、
/// コマンド受信と監視処理を交互に実行する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `rx` - ワーカーに操作を依頼するためのチャネル受信端
fn run_worker_loop(state: &Arc<AppState>, rx: &Receiver<ClipboardCommand>) {
    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            crate::log_error!("クリップボード初期化エラー: {:?}", e);
            platform::show_notification(
                "クリップボードエラー",
                "クリップボードの初期化に失敗しました。監視処理は停止します。",
            );
            return;
        }
    };

    let watcher = ChangeWatcher::new();
    let mut monitor = MonitorLoopState::new();
    let mut last_config_poll = Instant::now();

    loop {
        sync_monitor_generation(&mut monitor, &mut clipboard, state, &watcher);

        if last_config_poll.elapsed() >= CONFIG_POLL_INTERVAL {
            last_config_poll = Instant::now();
            if state.has_external_config_change() {
                dispatch::send_app_event(&state.proxy, AppEvent::ReloadConfig);
            }
        }

        let timeout = monitor.recv_timeout(state, &watcher);
        match rx.recv_timeout(timeout) {
            Ok(cmd) => handle_command(&mut clipboard, state, &mut monitor.refine_ctx, cmd),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if should_run_monitor_tick(state, &monitor) {
            monitor.tick(&mut clipboard, state, &watcher);
        }
    }
}

/// 監視ループの1ティックを実行するかどうかを判定する
///
/// 監視が停止中の場合はfalseを返し、
/// 監視中は監視世代が一致している場合はtrueを返す
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `monitor` - 監視ループ状態
///
/// # Returns
/// * `bool` - 監視ループの1ティックを実行するかどうか
fn should_run_monitor_tick(state: &Arc<AppState>, monitor: &MonitorLoopState) -> bool {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    !state.with_config(|c| c.is_paused)
        && generation > 0
        && monitor.tracked_generation == generation
}

/// 監視世代を同期する
///
/// 現在のクリップボード本文を観測済みとして記録し、イベント監視用トークンと
/// ポーリング用タイマーをリセットする
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
/// クリップボードの設定や加工を行い、成功時に通知を表示する
///
/// # Arguments
/// * `clipboard` - クリップボード操作用のインスタンス
/// * `state` - アプリケーションの共有状態
/// * `refine_ctx` - 加工コンテキスト (正規表現コンパイルキャッシュを保持)
/// * `cmd` - 受信したコマンド
pub(crate) fn handle_command<C: TextClipboard + crate::refiner::ImageClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    refine_ctx: &mut RefineContext,
    cmd: ClipboardCommand,
) {
    match cmd {
        ClipboardCommand::SetText(text) => {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                platform::show_notification(
                    "クリップボードエラー",
                    "履歴からの復元処理に失敗しました。",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    platform::show_notification("履歴から復元", "クリップボードにコピーしました");
                }
            }
        }
        ClipboardCommand::CopyRegisteredText(text) => {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                platform::show_notification(
                    "クリップボードエラー",
                    "登録文字列のコピーに失敗しました。",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    platform::show_notification("登録文字列", "クリップボードにコピーしました");
                }
            }
        }
        ClipboardCommand::SetOcrText(text) => {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                platform::show_notification(
                    "OCR エラー",
                    "クリップボードへの書き込みに失敗しました",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    platform::show_notification("OCR", "クリップボードにコピーしました");
                }
            }
        }
        ClipboardCommand::ProcessMode(mode) => {
            let pre_text = clipboard.get_text().ok();
            refine_ctx.regex = state.with_config(|c| c.regex.clone());
            let pipeline = [mode];
            match process_clipboard_pipeline_io(clipboard, &pipeline, refine_ctx) {
                Ok(ClipboardProcessOutcome::Processed(processed)) => {
                    if let Some(ref pre) = pre_text {
                        state.record_undo_source(pre);
                    }
                    state.record_processing_success(&processed);
                    notify::show_process_notification(state, &pipeline, &processed);
                }
                Ok(ClipboardProcessOutcome::ImageProcessed { width, height }) => {
                    if let Some(ref pre) = pre_text {
                        state.record_undo_source(pre);
                        state.record_image_processing_success(pre);
                    }
                    notify::show_image_process_notification(state, mode, width, height);
                }
                Ok(ClipboardProcessOutcome::Unchanged) => {
                    if state.with_config(|c| c.notification_settings.enabled) {
                        platform::show_notification("加工結果", "テキストに変更はありませんでした");
                    }
                }
                Err(e) => {
                    crate::log_error!("加工エラー: {} ({:?})", e.user_message(), e);
                    platform::show_notification("加工エラー", e.user_message());
                }
            }
        }
        ClipboardCommand::Undo => {
            if let Some(text) = state.take_undo_source() {
                if let Err(e) = clipboard.set_text(text.to_string()) {
                    crate::log_error!("加工取り消しエラー: {:?}", e);
                    state.record_undo_source(&text);
                    platform::show_notification(
                        "クリップボードエラー",
                        "加工の取り消しに失敗しました",
                    );
                } else {
                    state.record_processing_success(&text);
                    if state.with_config(|c| c.notification_settings.enabled) {
                        platform::show_notification(
                            "加工の取り消し",
                            "クリップボードを加工前の内容に戻しました",
                        );
                    }
                }
            } else if state.with_config(|c| c.notification_settings.enabled) {
                platform::show_notification("加工の取り消し", "取り消せる加工がありません");
            }
        }
        ClipboardCommand::RegisterFromClipboard => {
            register_text_from_clipboard(clipboard, state);
        }
    }
}

/// クリップボードの内容を登録文字列として保存する
fn register_text_from_clipboard<C: TextClipboard>(clipboard: &mut C, state: &Arc<AppState>) {
    let text = match clipboard.get_text() {
        Ok(text) => text,
        Err(e) => {
            crate::log_error!("クリップボード読み取りエラー: {:?}", e);
            platform::show_notification(
                "クリップボードエラー",
                "クリップボードの読み取りに失敗しました",
            );
            return;
        }
    };

    let outcome = state.with_config_mut(|c| c.add_registered_text(text));
    match outcome {
        Ok(()) => {
            state.save_config();
            dispatch::send_app_event(&state.proxy, AppEvent::RefreshTexts);
            notify::show_when_enabled(state, "登録文字列", "クリップボードの内容を登録しました");
        }
        Err(AddRegisteredTextError::Empty) => {
            notify::show_when_enabled(
                state,
                "登録文字列",
                "クリップボードが空のため登録できません",
            );
        }
        Err(AddRegisteredTextError::TooLarge) => {
            notify::show_when_enabled(state, "登録文字列", "テキストが長すぎるため登録できません");
        }
        Err(AddRegisteredTextError::LimitReached) => {
            notify::show_when_enabled(state, "登録文字列", "登録件数の上限に達しています");
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::*;

    use crate::config::MonitorMode;
    use crate::tray::clipboard_change::ChangeWatcher;
    use crate::tray::state::test_app_state;

    fn active_monitor(generation: u64) -> MonitorLoopState {
        let mut monitor = MonitorLoopState::new();
        monitor.tracked_generation = generation;
        monitor
    }

    /// 監視中かつ世代が一致する場合はティックを実行すること
    #[test]
    fn should_run_monitor_tick_when_active() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);
        state.monitor_generation.store(2, Ordering::SeqCst);

        let monitor = active_monitor(2);
        assert!(should_run_monitor_tick(&state, &monitor));
    }

    /// 一時停止中はティックを実行しないこと
    #[test]
    fn should_not_run_monitor_tick_when_paused() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = true);
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 監視世代が 0 の場合はティックを実行しないこと
    #[test]
    fn should_not_run_monitor_tick_when_generation_zero() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);

        let monitor = active_monitor(0);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 追跡中の世代と不一致の場合はティックを実行しないこと
    #[test]
    fn should_not_run_monitor_tick_when_generation_mismatch() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);
        state.monitor_generation.store(2, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 一時停止中は `recv_timeout` が短い間隔を返すこと
    #[test]
    fn recv_timeout_is_short_when_paused() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| {
            c.is_paused = true;
            c.monitor_mode = MonitorMode::Polling;
            c.interval_ms = 5000;
        });
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        let watcher = ChangeWatcher::new();
        assert_eq!(
            monitor.recv_timeout(&state, &watcher),
            Duration::from_millis(100)
        );
    }

    /// 設定が Polling の場合は実効監視モードも Polling であること
    #[test]
    fn effective_mode_is_polling_when_configured() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Polling);

        let watcher = ChangeWatcher::new();
        assert_eq!(
            MonitorLoopState::effective_mode(&watcher, &state),
            MonitorMode::Polling
        );
    }

    /// Event 設定でもウォッチャー非対応時は Polling にフォールバックすること
    #[test]
    fn effective_mode_falls_back_when_event_unsupported() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Event);

        let watcher = ChangeWatcher::new();
        let effective = MonitorLoopState::effective_mode(&watcher, &state);

        if watcher.is_supported() {
            assert_eq!(effective, MonitorMode::Event);
        } else {
            assert_eq!(effective, MonitorMode::Polling);
        }
    }
}
