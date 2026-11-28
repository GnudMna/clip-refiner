use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::super::clipboard_monitor::{
    self, ClipboardObserveResult, ClipboardTrigger, EVENT_POLL_MS,
};
use super::super::state::AppState;
use super::clipboard_seq;
use crate::config::MonitorMode;
use crate::refiner::{OwnedClipboard, RefineContext, TextClipboard};

// ======================================================================
// 定数
// ======================================================================
/// 読み取り失敗後に再試行する間隔
const READ_RETRY_INTERVAL: Duration = Duration::from_millis(150);
/// 同じ変更トークンに対する読み取り失敗後の最大再試行回数
const READ_RETRY_MAX: u32 = 5;

// ======================================================================
// 監視ループ状態
// ======================================================================
/// ワーカースレッド内でクリップボード監視を行うための状態
///
/// 監視世代、ポーリング間隔、イベントトークンの追跡を担当する
pub(super) struct MonitorLoopState {
    /// 読み取り失敗を再試行中の変更トークン
    pub retry_token: Option<u64>,
    /// 読み取り失敗の残り再試行回数
    pub retries_left: u32,
    /// 次に読み取りを再試行する時刻 (再試行待ちでなければ `None`)
    pub retry_due: Option<Instant>,
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
            retry_token: None,
            retries_left: 0,
            retry_due: None,
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
    /// ポーリング用タイマーをリセットする。監視除外が指定された内容は本文を読まない
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
        self.clear_read_retry();
        self.last_poll_at = Instant::now();
        self.tracked_generation = state.monitor_generation.load(Ordering::SeqCst);
        self.event_fallback_warned = false;
    }

    /// 設定値と環境に応じた実効監視モードを返す
    ///
    /// イベント監視が利用できない場合はポーリング方式にフォールバックする
    ///
    /// # Arguments
    /// * `configured` - 設定上の監視方式
    ///
    /// # Returns
    /// * `MonitorMode` - 実際に使用する監視方式
    pub fn effective_mode(configured: MonitorMode) -> MonitorMode {
        match configured {
            MonitorMode::Event if clipboard_seq::supports_event_monitor() => MonitorMode::Event,
            MonitorMode::Event | MonitorMode::Polling => MonitorMode::Polling,
        }
    }

    /// コマンド待ちのタイムアウトを計算する
    ///
    /// 監視世代が未開始または追跡中の世代と不一致の場合は短い間隔で再確認する。Event / 連続記録でリスナが稼働中のときは
    /// 設定外部変更の安全網ポーリング間隔まで待ち、`ClipboardChanged` / `ConfigFileChanged` で起床する
    /// リスナ未使用時はシーケンストークンの短周期 peek にフォールバックする。
    /// Polling は次の周期まで待つ (監視世代と監視周期の変更は `Wake` で起床する)
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

        if snap.continuous_record_enabled
            || Self::effective_mode(snap.monitor_mode) == MonitorMode::Event
        {
            if listener_active {
                // 読み取り失敗の再試行待ちは、次のクリップボード変更や安全網を待たず短い間隔で起床する
                return match self.retry_due {
                    Some(due) => idle_timeout
                        .min(due.saturating_duration_since(Instant::now()))
                        .max(Duration::from_millis(1)),
                    None => idle_timeout,
                };
            }
            return Duration::from_millis(EVENT_POLL_MS);
        }

        let elapsed = u64::try_from(self.last_poll_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let remaining = snap.interval_ms.saturating_sub(elapsed);
        Duration::from_millis(remaining.max(1))
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

        if snap.monitor_mode == MonitorMode::Event
            && !clipboard_seq::supports_event_monitor()
            && !self.event_fallback_warned
        {
            crate::log_warn!(
                "イベント監視が利用できないため、ポーリング監視にフォールバックします"
            );
            self.event_fallback_warned = true;
        }

        if snap.continuous_record_enabled {
            self.tick_continuous_record(clipboard, state, &snap);
        }

        if !snap.auto_processing_enabled() {
            Self::discard_unconsumed_last_written(state, &snap);
            return;
        }

        match Self::effective_mode(snap.monitor_mode) {
            MonitorMode::Event => self.tick_event_processing(clipboard, state, &snap),
            MonitorMode::Polling => self.tick_polling_processing(clipboard, state, &snap),
        }
    }

    /// 自動加工も連続記録も無効な間に残った `last_written` を破棄する
    ///
    /// 判定側が消費しないため、手動加工や履歴復元で記録された指紋が残り続ける
    /// 残すと後から自動変換を有効にして同じ文字列をコピーしたとき、書き戻しのエコーと誤認して加工されない
    fn discard_unconsumed_last_written(
        state: &Arc<AppState>,
        snap: &super::super::state::MonitorSnapshot,
    ) {
        if snap.continuous_record_enabled {
            return;
        }
        state
            .clipboard_session()
            .with_processed_state(|ps| ps.last_written = None);
    }

    /// 連続記録向けにクリップボード変更を検知する
    ///
    /// 監視方式に依存せず変更トークンで検知する (`ClipboardChanged` 起床後も同じ判定)
    /// トークンを取得できない環境では `interval_ms` 間隔で本文を観測する
    fn tick_continuous_record(
        &mut self,
        clipboard: &mut OwnedClipboard,
        state: &Arc<AppState>,
        snap: &super::super::state::MonitorSnapshot,
    ) {
        if let Some(token) = clipboard_seq::token() {
            if token != self.last_token {
                let result = clipboard_monitor::observe_continuous_record(clipboard, state);
                Self::apply_token_after_observe(self, state, token, result);
            }
            return;
        }

        if self.last_poll_at.elapsed() >= Duration::from_millis(snap.interval_ms) {
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

        state.sync_regex_settings(&mut self.refine_ctx.regex);
        let result = clipboard_monitor::handle_clipboard_update(
            clipboard,
            state,
            snap,
            ClipboardTrigger::Event,
            &self.refine_ctx,
        );
        Self::apply_token_after_observe(self, state, token, result);
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
        state.sync_regex_settings(&mut self.refine_ctx.regex);
        let result = clipboard_monitor::handle_clipboard_update(
            clipboard,
            state,
            snap,
            trigger,
            &self.refine_ctx,
        );
        // 読み取り失敗でも次回はポーリング間隔を空ける (OpenClipboard の連続再試行を避ける)
        self.last_poll_at = Instant::now();
        if let Some(token) = token {
            Self::apply_token_after_observe(self, state, token, result);
        }
    }

    /// 読み取り失敗後の再試行を予約する
    ///
    /// 変更トークンは進めないため、次のティックで同じ変更を再び読む
    /// 別のトークンの失敗は再試行回数を数え直し、上限に達したら予約しない
    fn schedule_read_retry(&mut self, token: u64) {
        if self.retry_token != Some(token) {
            self.retry_token = Some(token);
            self.retries_left = READ_RETRY_MAX;
        }
        if self.retries_left == 0 {
            self.retry_due = None;
            return;
        }
        self.retries_left -= 1;
        self.retry_due = Some(Instant::now() + READ_RETRY_INTERVAL);
    }

    /// 読み取り失敗の再試行状態を破棄する
    fn clear_read_retry(&mut self) {
        self.retry_token = None;
        self.retries_left = 0;
        self.retry_due = None;
    }

    /// 観測結果に応じて変更トークンを更新する
    fn apply_token_after_observe(
        monitor: &mut Self,
        state: &Arc<AppState>,
        token: u64,
        result: ClipboardObserveResult,
    ) {
        Self::apply_token_with_current(monitor, state, token, result, clipboard_seq::token());
    }

    /// 書き戻し後の現在トークンを受け取って変更トークンを更新する
    ///
    /// 書き戻しを反映したトークンを取得できた場合、自身の書き戻しによる変更はトークン一致で
    /// 読み飛ばされ判定側の `last_written` が消費されないため、ここで消す
    /// 消さないと直後に同じ文字列をコピーしたとき、書き戻しのエコーと誤認して加工されない
    fn apply_token_with_current(
        monitor: &mut Self,
        state: &Arc<AppState>,
        token: u64,
        result: ClipboardObserveResult,
        current_token: Option<u64>,
    ) {
        match result {
            ClipboardObserveResult::ReadFailed => monitor.schedule_read_retry(token),
            ClipboardObserveResult::Observed => {
                monitor.clear_read_retry();
                monitor.last_token = token;
            }
            ClipboardObserveResult::ClipboardWritten => {
                monitor.clear_read_retry();
                monitor.last_token = current_token.unwrap_or(token);
                if current_token.is_some() {
                    state
                        .clipboard_session()
                        .with_processed_state(|ps| ps.last_written = None);
                }
            }
        }
    }
}

/// 監視ループの1ティックを実行するかどうかを判定する
///
/// 監視世代が未開始、または追跡中の世代と一致しない間は実行しない
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `monitor` - 監視ループ状態
///
/// # Returns
/// * `bool` - 監視ループの1ティックを実行するかどうか
pub(super) fn should_run_monitor_tick(state: &Arc<AppState>, monitor: &MonitorLoopState) -> bool {
    let generation = state.monitor_generation.load(Ordering::SeqCst);
    generation != 0 && monitor.tracked_generation == generation
}

/// 監視世代を同期する
///
/// 監視世代が 0 なら追跡中の世代を 0 に戻す。追跡中の世代と異なる場合だけ
/// `MonitorLoopState::reset` でベースライン (観測済み本文・トークン・ポーリング用タイマー) を再同期する
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

    if generation == 0 {
        monitor.tracked_generation = 0;
        return;
    }

    if monitor.tracked_generation != generation {
        monitor.reset(clipboard, state);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::ContentFingerprint;
    use crate::tray::state::test_app_state;

    /// `last_written` を持つ状態と監視ループを生成する
    fn state_with_last_written() -> (Arc<AppState>, MonitorLoopState) {
        let state = Arc::new(test_app_state());
        state.clipboard_session().with_processed_state(|ps| {
            ps.last_written = Some(ContentFingerprint::from_text("out"));
        });
        (state, MonitorLoopState::new())
    }

    /// 書き戻し後のトークンを取得できた場合は `last_written` を消してトークンを進めること
    ///
    /// エコーはトークン一致で読み飛ばされ判定側で消費されないため、ここで消さないと
    /// 直後の同じ文字列のコピーが加工されない
    #[test]
    fn written_clears_last_written_when_token_available() {
        let (state, mut monitor) = state_with_last_written();

        MonitorLoopState::apply_token_with_current(
            &mut monitor,
            &state,
            5,
            ClipboardObserveResult::ClipboardWritten,
            Some(6),
        );

        // 書き戻し後のトークンへ進み、エコー用の記録は残さない
        assert_eq!(monitor.last_token, 6);
        let remains = state
            .clipboard_session()
            .with_processed_state(|ps| ps.matches_last_written("out"));
        assert!(!remains);
    }

    /// 書き戻し後のトークンを取得できない場合は `last_written` を残すこと
    ///
    /// エコーを判定側で読み飛ばす必要があるため
    #[test]
    fn written_keeps_last_written_when_token_unavailable() {
        let (state, mut monitor) = state_with_last_written();

        MonitorLoopState::apply_token_with_current(
            &mut monitor,
            &state,
            5,
            ClipboardObserveResult::ClipboardWritten,
            None,
        );

        assert_eq!(monitor.last_token, 5);
        let remains = state
            .clipboard_session()
            .with_processed_state(|ps| ps.matches_last_written("out"));
        assert!(remains);
    }

    /// 自動変換が無効な間は `last_written` を破棄すること
    ///
    /// 手動加工や履歴復元の指紋が残ると、後から自動変換を有効にした直後の同じ文字列のコピーが加工されない
    #[test]
    fn discards_last_written_while_auto_processing_disabled() {
        let (state, _) = state_with_last_written();
        state.with_config_mut(|c| {
            c.auto_refine_enabled = false;
            c.continuous_record_enabled = false;
        });
        let snap = state.monitor_snapshot();

        MonitorLoopState::discard_unconsumed_last_written(&state, &snap);

        let remains = state
            .clipboard_session()
            .with_processed_state(|ps| ps.matches_last_written("out"));
        assert!(!remains);
    }

    /// 連続記録中は `last_written` を破棄しないこと
    ///
    /// 連続記録の判定側がエコーの読み飛ばしに使うため
    #[test]
    fn keeps_last_written_while_continuous_record_enabled() {
        let (state, _) = state_with_last_written();
        state.with_config_mut(|c| {
            c.auto_refine_enabled = false;
            c.continuous_record_enabled = true;
        });
        let snap = state.monitor_snapshot();

        MonitorLoopState::discard_unconsumed_last_written(&state, &snap);

        let remains = state
            .clipboard_session()
            .with_processed_state(|ps| ps.matches_last_written("out"));
        assert!(remains);
    }

    /// 読み取り失敗後は上限まで再試行を予約し、成功で破棄すること
    ///
    /// トークンは進めず、上限到達後は次の変更まで予約しない
    #[test]
    fn read_failed_schedules_bounded_retries() {
        let (state, mut monitor) = state_with_last_written();

        for _ in 0..READ_RETRY_MAX {
            MonitorLoopState::apply_token_with_current(
                &mut monitor,
                &state,
                5,
                ClipboardObserveResult::ReadFailed,
                Some(5),
            );
            assert!(monitor.retry_due.is_some());
        }
        MonitorLoopState::apply_token_with_current(
            &mut monitor,
            &state,
            5,
            ClipboardObserveResult::ReadFailed,
            Some(5),
        );
        assert_eq!(monitor.last_token, 0);
        assert!(monitor.retry_due.is_none());

        // 別のトークンでは再び再試行する
        MonitorLoopState::apply_token_with_current(
            &mut monitor,
            &state,
            6,
            ClipboardObserveResult::ReadFailed,
            Some(6),
        );
        assert!(monitor.retry_due.is_some());

        MonitorLoopState::apply_token_with_current(
            &mut monitor,
            &state,
            6,
            ClipboardObserveResult::Observed,
            Some(6),
        );
        assert!(monitor.retry_due.is_none());
        assert_eq!(monitor.last_token, 6);
    }

    /// 観測のみの場合は `last_written` を変更しないこと
    #[test]
    fn observed_keeps_last_written() {
        let (state, mut monitor) = state_with_last_written();

        MonitorLoopState::apply_token_with_current(
            &mut monitor,
            &state,
            5,
            ClipboardObserveResult::Observed,
            Some(6),
        );

        assert_eq!(monitor.last_token, 5);
        let remains = state
            .clipboard_session()
            .with_processed_state(|ps| ps.matches_last_written("out"));
        assert!(remains);
    }
}
