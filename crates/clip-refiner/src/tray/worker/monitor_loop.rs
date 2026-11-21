use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::super::clipboard_monitor::{self, EVENT_POLL_MS, POLL_TICK_MS};
use super::super::state::AppState;
use super::clipboard_seq;

use crate::config::MonitorMode;
use crate::refiner::RefineContext;

use arboard::Clipboard;

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
    /// ポーリング用タイマーをリセットする
    ///
    /// # Arguments
    /// * `clipboard` - クリップボード操作用のインスタンス
    /// * `state` - アプリケーションの共有状態
    pub fn reset(&mut self, clipboard: &mut Clipboard, state: &Arc<AppState>) {
        let text = clipboard.get_text().unwrap_or_default();
        state.record_clipboard_observed(&text);
        self.last_token = clipboard_seq::token().unwrap_or(0);
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
    /// 監視が停止中の場合は短い間隔で再確認し、
    /// 監視中はポーリング間隔またはイベント監視間隔に応じた値を返す
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `Duration` - `recv_timeout` に渡す待機時間
    pub fn recv_timeout(&self, state: &Arc<AppState>) -> Duration {
        let generation = state.monitor_generation.load(Ordering::SeqCst);
        if state.with_config(|c| c.is_paused)
            || generation == 0
            || self.tracked_generation != generation
        {
            return Duration::from_millis(100);
        }

        match Self::effective_mode(state) {
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
    pub fn tick(&mut self, clipboard: &mut Clipboard, state: &Arc<AppState>) {
        let snap = state.monitor_snapshot();
        if snap.is_paused {
            return;
        }

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

        match Self::effective_mode(state) {
            MonitorMode::Event => {
                if let Some(token) = clipboard_seq::token()
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
                        self.last_token = clipboard_seq::token().unwrap_or(self.last_token);
                    }
                }
            }
            MonitorMode::Polling => {
                if self.last_poll_at.elapsed() >= Duration::from_millis(snap.interval_ms) {
                    if snap.continuous_record_enabled {
                        // 連続記録は同一内容の再コピーでも積むため、変更トークンで検知する
                        if let Some(token) = clipboard_seq::token()
                            && token != self.last_token
                        {
                            self.last_token = token;
                            clipboard_monitor::handle_clipboard_update(
                                clipboard,
                                state,
                                &snap,
                                true,
                                &self.refine_ctx,
                            );
                        }
                    } else {
                        clipboard_monitor::handle_clipboard_update(
                            clipboard,
                            state,
                            &snap,
                            false,
                            &self.refine_ctx,
                        );
                    }
                    self.last_poll_at = Instant::now();
                }
            }
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
pub(super) fn should_run_monitor_tick(state: &Arc<AppState>, monitor: &MonitorLoopState) -> bool {
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
pub(super) fn sync_monitor_generation(
    monitor: &mut MonitorLoopState,
    clipboard: &mut Clipboard,
    state: &Arc<AppState>,
) {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    let paused = state.with_config(|c| c.is_paused);

    if paused || generation == 0 {
        monitor.tracked_generation = 0;
        return;
    }

    if monitor.tracked_generation != generation {
        monitor.reset(clipboard, state);
    }
}
