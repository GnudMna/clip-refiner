#![allow(clippy::missing_panics_doc)]

use std::sync::Arc;

use super::InMemoryTextClipboard;

use crate::config::{AppConfig, MonitorMode};
use crate::refiner::text_clipboard::TextClipboard;
use crate::refiner::{RefineContext, RefineMode};
use crate::security::secret_from;
use crate::tray::clipboard_monitor::handle_clipboard_update;
use crate::tray::state::test_app_state;
use crate::tray::try_paste_continuous_record_front;
use crate::tray::worker::{ClipboardCommand, handle_command};

// ======================================================================
// 統合テスト用ハーネス
// ======================================================================
/// 監視ループ・ワーカー経路を横断検証するためのテスト用ハーネス
pub struct ClipboardHarness {
    clipboard: InMemoryTextClipboard,
    state: Arc<crate::tray::state::AppState>,
    refine_ctx: RefineContext,
}

impl ClipboardHarness {
    /// 指定テキストを保持するハーネスを生成する
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            clipboard: InMemoryTextClipboard::with_text(text),
            state: Arc::new(test_app_state()),
            refine_ctx: RefineContext::default(),
        }
    }

    /// 加工モードを設定する
    #[must_use]
    pub fn with_mode(self, mode: RefineMode) -> Self {
        self.set_mode(mode);
        self
    }

    /// 履歴の有効・無効を設定する
    #[must_use]
    pub fn with_history(self, enabled: bool) -> Self {
        self.state.with_config_mut(|c| c.history_enabled = enabled);
        self
    }

    /// 連続記録モードの有効・無効を設定する
    #[must_use]
    pub fn with_continuous_record(self, enabled: bool) -> Self {
        self.state
            .with_config_mut(|c| c.continuous_record_enabled = enabled);
        self
    }

    /// 監視の一時停止フラグを設定する
    #[must_use]
    pub fn with_paused(self, paused: bool) -> Self {
        self.state.with_config_mut(|c| c.is_paused = paused);
        self
    }

    /// 監視方式を設定する
    #[must_use]
    pub fn with_monitor_mode(self, mode: MonitorMode) -> Self {
        self.state.with_config_mut(|c| c.monitor_mode = mode);
        self
    }

    /// 監視時に適用する加工パイプラインを設定する
    #[must_use]
    pub fn with_pipeline(self, pipeline: Vec<RefineMode>) -> Self {
        self.state.with_config_mut(|c| c.pipeline = pipeline);
        self
    }

    /// 加工モードを変更する
    pub fn set_mode(&self, mode: RefineMode) {
        self.state.with_config_mut(|c| c.mode = mode);
    }

    /// クリップボード上のテキストを返す
    pub fn clipboard_text(&self) -> &str {
        self.clipboard.text()
    }

    /// クリップボード上のテキストを直接置き換える (外部コピーを模倣)
    pub fn replace_clipboard(&mut self, text: impl Into<String>) {
        self.clipboard
            .set_text(text.into())
            .expect("クリップボードの置換に失敗");
    }

    /// インメモリクリップボードを新しい内容で初期化する
    pub fn reset_clipboard(&mut self, text: impl Into<String>) {
        self.clipboard = InMemoryTextClipboard::with_text(text);
    }

    /// 履歴件数を返す
    pub fn history_len(&self) -> usize {
        self.state.history_len()
    }

    /// 連続記録件数を返す
    pub fn continuous_record_len(&self) -> usize {
        self.state.continuous_record_len()
    }

    /// 履歴エントリを先頭 (最新) から順に復号して返す
    pub fn history_entries(&self) -> Vec<String> {
        (0..self.history_len())
            .filter_map(|i| self.history_entry_text(i))
            .collect()
    }

    /// 連続記録エントリを先頭 (キュー先頭) から順に復号して返す
    pub fn continuous_record_entries(&self) -> Vec<String> {
        (0..self.continuous_record_len())
            .filter_map(|i| self.continuous_record_entry_text(i))
            .collect()
    }

    /// 指定インデックスの履歴本文を返す
    pub fn history_entry_text(&self, index: usize) -> Option<String> {
        self.state.get_history_entry(index).map(|s| s.to_string())
    }

    /// 指定インデックスの連続記録本文を返す
    pub fn continuous_record_entry_text(&self, index: usize) -> Option<String> {
        self.state
            .get_continuous_record_entry(index)
            .map(|s| s.to_string())
    }

    /// 直近の加工前テキストを取り出す
    pub fn take_undo_source(&self) -> Option<String> {
        self.state.take_undo_source().map(|s| s.to_string())
    }

    /// 直近の書き戻し指紋が指定テキストと一致するか判定する
    pub fn matches_last_written(&self, text: &str) -> bool {
        self.state
            .with_processed_state(|ps| ps.matches_last_written(text))
    }

    /// 設定を変更する
    pub fn with_config_mut<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> R {
        self.state.with_config_mut(f)
    }

    /// 監視ループのクリップボード更新処理を実行する
    ///
    /// `event_driven` は OS イベント通知を受けたかどうかを表し、
    /// `MonitorMode::Event` 設定時のワーカー挙動と一致させる場合は
    /// [`Self::run_configured_monitor_update`] を使う
    pub fn run_monitor_update(&mut self, event_driven: bool) -> bool {
        let snap = self.state.monitor_snapshot();
        if snap.is_paused {
            return false;
        }
        self.refine_ctx.regex = snap.regex_settings.clone();
        handle_clipboard_update(
            &mut self.clipboard,
            &self.state,
            &snap,
            event_driven,
            &self.refine_ctx,
        )
    }

    /// 設定の `monitor_mode` に応じた `event_driven` フラグで監視更新を実行する
    pub fn run_configured_monitor_update(&mut self) -> bool {
        let event_driven = self
            .state
            .with_config(|c| c.monitor_mode == MonitorMode::Event);
        self.run_monitor_update(event_driven)
    }

    /// ワーカーへ送られる `ProcessMode` コマンドを処理する
    pub fn process_mode(&mut self, mode: RefineMode) {
        self.refine_ctx.regex = self.state.with_config(|c| c.regex.clone());
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::ProcessMode(mode),
        );
    }

    /// ワーカーへ送られる `SetText` コマンドを処理する
    pub fn set_text(&mut self, text: impl AsRef<str>) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::SetText(secret_from(text.as_ref().to_string())),
        );
    }

    /// ワーカーへ送られる `SetOcrText` コマンドを処理する
    pub fn set_ocr_text(&mut self, text: impl AsRef<str>) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::SetOcrText(secret_from(text.as_ref().to_string())),
        );
    }

    /// ワーカーへ送られる `Undo` コマンドを処理する
    pub fn undo(&mut self) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::Undo,
        );
    }

    /// 貼り付け (`Ctrl+V`) 相当でキュー先頭をクリップボードへ載せる
    pub fn discharge_continuous_record_on_paste(&mut self) {
        let _ = try_paste_continuous_record_front(&self.state, &mut self.clipboard);
    }

    /// 連続記録キューから指定位置を取り出してクリップボードへセットする
    pub fn discharge_continuous_record_at(&mut self, index: usize) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::DischargeContinuousRecordAt(index),
        );
    }

    /// 有効化直後に現在のクリップボード内容をキューへ積む
    pub fn catch_up_continuous_record(&mut self) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::CatchUpContinuousRecord,
        );
    }

    /// ワーカーへ送られる `RegisterClipFromClipboard` コマンドを処理する
    pub fn register_clip_from_clipboard(&mut self) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::RegisterClipFromClipboard,
        );
    }

    /// ワーカーへ送られる `CopyRegisteredClip` コマンドを処理する
    pub fn copy_registered_clip(&mut self, index: usize) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::CopyRegisteredClip(index),
        );
    }

    /// 登録クリップ件数を返す
    pub fn registered_clip_count(&self) -> usize {
        self.state.with_config(|c| c.clips.len())
    }

    /// 指定インデックスの登録クリップ本文を返す (テキスト登録のみ)
    pub fn registered_clip_text(&self, index: usize) -> Option<String> {
        self.state
            .with_config(|c| c.resolve_registered_clip(index))
            .and_then(|clip| match clip {
                crate::config::ResolvedClip::Text(text) => Some(text),
                crate::config::ResolvedClip::Image { .. } => None,
            })
    }
}
