use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::super::clipboard_monitor::{
    self, ClipboardObserveResult, ClipboardTrigger, EVENT_POLL_MS, POLL_TICK_MS,
};
use super::super::state::AppState;
use super::clipboard_seq;
use crate::config::MonitorMode;
use crate::refiner::{OwnedClipboard, RefineContext, TextClipboard};

// ======================================================================
// 監視ループ状態
// ======================================================================
/// ワーカースレッド内でクリップボード監視を行うための状態
///
/// 監視世代、ポーリング間隔、イベントトークンの追跡を担当する
pub(super) struct MonitorLoopState {
    /// 現在追跡中の監視世代(0 は監視停止中)
    pub tracked_generation: u64,
    /// 前回ポーリング監視を実行した時刻
    pub last_poll_at: Instant,
    /// イベント監視用の前回トークン
    pub last_token: u64,
    /// イベント監視フォールバック警告を既に出力したか
    pub event_fallback_warned: bool,
    /// 加工コンテキスト (正規表現コンパイルキャッシュを保持)
    pub refine_ctx: RefineContext,
}

impl MonitorLoopState {
    /// 新しい監視ループ状態を生成する
    ///
    /// # Returns
    /// * `Self` - 監視停止状態の初期インスタンス
    pub fn new() -> Self {
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
    /// ポーリング用タイマーをリセットする。監視除外が指定された内容は本文を読まない。
    ///
    /// # Arguments
    /// * `clipboard` - クリップボード操作用のインスタンス
    /// * `state` - アプリケーションの共有状態
    pub fn reset(&mut self, clipboard: &mut OwnedClipboard, state: &Arc<AppState>) {
        let new_token = clipboard_seq::token().unwrap_or(0);
        if !clipboard.is_excluded_from_monitoring() {
            let text = clipboard.get_text().unwrap_or_default();
            let token_changed = new_token != self.last_token;
            let initial_sync = self.tracked_generation == 0;
            let already_seen = state
                .clipboard_session()
                .with_processed_state(|ps| ps.matches_last_seen(&text));
            state.clipboard_session().record_clipboard_observed(&text);
            clipboard_monitor::recover_continuous_record_on_reset(
                state,
                &text,
                token_changed,
                already_seen,
                initial_sync,
            );
        }
        self.last_token = new_token;
        self.last_poll_at = Instant::now();
        self.tracked_generation = state.monitor_generation.load(Ordering::SeqCst);
        self.event_fallback_warned = false;
    }

    /// 設定と環境に応じた実効監視モードを返す
    ///
    /// イベント監視が利用できない場合はポーリング方式にフォールバックする
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `MonitorMode` - 実際に使用する監視方式
    pub fn effective_mode(state: &Arc<AppState>) -> MonitorMode {
        match state.with_config(|c| c.monitor_mode) {
            MonitorMode::Event if clipboard_seq::supports_event_monitor() => MonitorMode::Event,
            MonitorMode::Event | MonitorMode::Polling => MonitorMode::Polling,
        }
    }

    /// コマンド待ちのタイムアウトを計算する
    ///
    /// 監視が停止中の場合は短い間隔で再確認する。Event / 連続記録でリスナが稼働中のときは
    /// 設定外部変更の安全網ポーリング間隔まで待ち、`ClipboardChanged` / `ConfigFileChanged` で起床する
    /// リスナ未使用時はシーケンストークンの短周期 peek にフォールバックする
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    /// * `listener_active` - OS クリップボードリスナが稼働中なら `true`
    /// * `idle_timeout` - リスナ稼働時の待機上限 (設定外部変更検知用)
    ///
    /// # Returns
    /// * `Duration` - `recv_timeout` に渡す待機時間
    pub fn recv_timeout(
        &self,
        state: &Arc<AppState>,
        listener_active: bool,
        idle_timeout: Duration,
    ) -> Duration {
        let generation = state.monitor_generation.load(Ordering::SeqCst);
        let snap = state.monitor_snapshot();
        if generation == 0 || self.tracked_generation != generation {
            return Duration::from_millis(100);
        }

        if snap.is_paused && !snap.continuous_record_enabled {
            return Duration::from_millis(100);
        }

        if snap.continuous_record_enabled || Self::effective_mode(state) == MonitorMode::Event {
            if listener_active {
                return idle_timeout;
            }
            return Duration::from_millis(POLL_TICK_MS.min(EVENT_POLL_MS));
        }

        let elapsed = u64::try_from(self.last_poll_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let remaining = snap.interval_ms.saturating_sub(elapsed);
        Duration::from_millis(POLL_TICK_MS.min(remaining.max(1)))
    }

    /// 監視ループの1ティックを実行する
    ///
    /// 実効監視モードに応じてクリップボードの変更を検知し、
    /// 必要に応じて加工処理を委譲する
    ///
    /// # Arguments
    /// * `clipboard` - クリップボード操作用のインスタンス
    /// * `state` - アプリケーションの共有状態
    pub fn tick(&mut self, clipboard: &mut OwnedClipboard, state: &Arc<AppState>) {
        let snap = state.monitor_snapshot();

        if state.with_config(|c| c.monitor_mode) == MonitorMode::Event
            && !clipboard_seq::supports_event_monitor()
            && !self.event_fallback_warned
        {
            crate::log_warn!(
                "イベント監視が利用できないため、ポーリング監視にフォールバックします"
            );
            self.event_fallback_warned = true;
        }

        self.refine_ctx.regex = snap.regex_settings.clone();

        if snap.continuous_record_enabled {
            self.tick_continuous_record(clipboard, state);
        }

        if !snap.auto_processing_enabled() {
            return;
        }

        match Self::effective_mode(state) {
            MonitorMode::Event => self.tick_event_processing(clipboard, state, &snap),
            MonitorMode::Polling => self.tick_polling_processing(clipboard, state, &snap),
        }
    }

    /// 連続記録向けにクリップボード変更を検知する
    ///
    /// 監視方式に依存せず変更トークンで検知する (`ClipboardChanged` 起床後も同じ判定)
    fn tick_continuous_record(&mut self, clipboard: &mut OwnedClipboard, state: &Arc<AppState>) {
        if let Some(token) = clipboard_seq::token() {
            if token != self.last_token {
                let result = clipboard_monitor::observe_continuous_record(clipboard, state);
                Self::apply_token_after_observe(self, token, result);
            }
            return;
        }

        let interval_ms = state.with_config(|c| c.interval_ms);
        if self.last_poll_at.elapsed() >= Duration::from_millis(interval_ms) {
            let _ = clipboard_monitor::observe_continuous_record(clipboard, state);
            self.last_poll_at = Instant::now();
        }
    }

    /// イベント監視で加工処理を試みる
    fn tick_event_processing(
        &mut self,
        clipboard: &mut OwnedClipboard,
        state: &Arc<AppState>,
        snap: &super::super::state::MonitorSnapshot,
    ) {
        let Some(token) = clipboard_seq::token() else {
            return;
        };
        if token == self.last_token {
            return;
        }

        let result = clipboard_monitor::handle_clipboard_update(
            clipboard,
            state,
            snap,
            ClipboardTrigger::Event,
            &self.refine_ctx,
        );
        Self::apply_token_after_observe(self, token, result);
    }

    /// ポーリング監視で加工処理を試みる
    ///
    /// 変更トークンが前回から進んでいなければ本文を読まない。テキストの無い画像は
    /// 本文で重複を判定できないため、トークンで変更を確認しないと同じ画像を加工し続ける
    fn tick_polling_processing(
        &mut self,
        clipboard: &mut OwnedClipboard,
        state: &Arc<AppState>,
        snap: &super::super::state::MonitorSnapshot,
    ) {
        if self.last_poll_at.elapsed() < Duration::from_millis(snap.interval_ms) {
            return;
        }

        let token = clipboard_seq::token();
        if token == Some(self.last_token) {
            self.last_poll_at = Instant::now();
            return;
        }

        let trigger = if token.is_some() {
            ClipboardTrigger::PollingChanged
        } else {
            ClipboardTrigger::PollingUnconfirmed
        };
        let result = clipboard_monitor::handle_clipboard_update(
            clipboard,
            state,
            snap,
            trigger,
            &self.refine_ctx,
        );
        if result != ClipboardObserveResult::ReadFailed {
            self.last_poll_at = Instant::now();
        }
        if let Some(token) = token {
            Self::apply_token_after_observe(self, token, result);
        }
    }

    /// 観測結果に応じて変更トークンを更新する
    fn apply_token_after_observe(monitor: &mut Self, token: u64, result: ClipboardObserveResult) {
        match result {
            ClipboardObserveResult::ReadFailed => {}
            ClipboardObserveResult::Observed => monitor.last_token = token,
            ClipboardObserveResult::ClipboardWritten => {
                monitor.last_token = clipboard_seq::token().unwrap_or(token);
            }
        }
    }
}

/// 監視ループの1ティックを実行するかどうかを判定する
///
/// 連続記録が有効なら一時停止中でもティックを実行する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `monitor` - 監視ループ状態
///
/// # Returns
/// * `bool` - 監視ループの1ティックを実行するかどうか
pub(super) fn should_run_monitor_tick(state: &Arc<AppState>, monitor: &MonitorLoopState) -> bool {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    if generation == 0 || monitor.tracked_generation != generation {
        return false;
    }

    state.with_config(|c| !c.is_paused || c.continuous_record_enabled)
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
pub(super) fn sync_monitor_generation(
    monitor: &mut MonitorLoopState,
    clipboard: &mut OwnedClipboard,
    state: &Arc<AppState>,
) {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    let paused = state.with_config(|c| c.is_paused);
    let continuous_record = state.with_config(|c| c.continuous_record_enabled);

    if generation == 0 || (paused && !continuous_record) {
        monitor.tracked_generation = 0;
        return;
    }

    if monitor.tracked_generation != generation {
        monitor.reset(clipboard, state);
    }
}
