#![allow(
    clippy::missing_panics_doc,
    reason = "テスト専用ヘルパーで、失敗時の panic は仕様"
)]

use std::sync::Arc;

use super::InMemoryTextClipboard;
use crate::config::{AppConfig, MonitorMode};
use crate::refiner::text_clipboard::{ImageClipboard, TextClipboard};
use crate::refiner::{RefineContext, RefineMode};
use crate::security::secret_from;
use crate::tray::clipboard_monitor::{
    ClipboardObserveResult, ClipboardTrigger, handle_clipboard_update, observe_continuous_record,
};
use crate::tray::state::test_app_state;
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
    ///
    /// # Arguments
    /// * `text` - インメモリクリップボードの初期内容
    ///
    /// # Returns
    /// * `Self` - 生成された `ClipboardHarness`
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            clipboard: InMemoryTextClipboard::with_text(text),
            state: Arc::new(test_app_state()),
            refine_ctx: RefineContext::default(),
        }
    }

    /// クリップボードの内容をパスワードマネージャー等の除外形式付きにする
    ///
    /// # Returns
    /// * `Self` - 除外形式付きのクリップボードを持つ `ClipboardHarness`
    #[must_use]
    pub fn with_excluded_clipboard(mut self) -> Self {
        self.clipboard = self.clipboard.excluded_from_monitoring();
        self
    }

    /// クリップボードが除外形式付きかを返す
    ///
    /// # Returns
    /// * `bool` - 除外形式付きの場合は `true`
    pub fn clipboard_is_excluded(&mut self) -> bool {
        self.clipboard.is_excluded_from_monitoring()
    }

    /// クリップボードに HTML 形式 (`CF_HTML` の断片) を同時に置く
    ///
    /// # Arguments
    /// * `html` - HTML 断片
    ///
    /// # Returns
    /// * `Self` - HTML を置いた `self`
    #[must_use]
    pub fn with_html(mut self, html: impl Into<String>) -> Self {
        let text = self.clipboard.text().to_string();
        self.clipboard = InMemoryTextClipboard::with_text(text).with_html(html);
        self
    }

    /// クリップボードに読み取り元の画像を同時に置く
    ///
    /// # Arguments
    /// * `width` - 画像幅 (ピクセル)
    /// * `height` - 画像高さ (ピクセル)
    /// * `rgba` - RGBA ピクセル列
    ///
    /// # Returns
    /// * `Self` - 画像を置いた `self`
    #[must_use]
    pub fn with_source_image(mut self, width: u32, height: u32, rgba: Vec<u8>) -> Self {
        let clipboard =
            std::mem::replace(&mut self.clipboard, InMemoryTextClipboard::with_text(""));
        self.clipboard = clipboard.with_source_image(width, height, rgba);
        self
    }

    /// 直近の画像が、ファイルとしても貼り付けられる形で書き込まれたかを返す
    ///
    /// # Returns
    /// * `bool` - ファイルとしても貼り付けられる場合は `true`
    pub fn image_written_as_file(&self) -> bool {
        self.clipboard.image_written_as_file()
    }

    /// ほかのアプリが画像をコピーし、変更トークンが進んだ状態にする
    pub fn copy_external_image(&mut self) {
        let (width, height, rgba) = self
            .clipboard
            .get_image()
            .expect("読み取り元の画像が置かれていること");
        self.clipboard
            .set_image(width, height, rgba)
            .expect("画像の書き込み");
    }

    /// 書き込み後の画像サイズを返す
    ///
    /// # Returns
    /// * `Option<(u32, u32)>` - 書き込み済み画像の幅と高さ。未書き込みの場合は `None`
    pub fn written_image_size(&self) -> Option<(u32, u32)> {
        self.clipboard.written_image_size()
    }

    /// 本文の読み取り直後に別のコピーが入るよう予約する
    ///
    /// # Arguments
    /// * `text` - 割り込むコピーのテキスト
    ///
    /// # Returns
    /// * `Self` - 割り込みコピーを予約した `self`
    #[must_use]
    pub fn with_external_copy_after_read(mut self, text: impl Into<String>) -> Self {
        let clipboard =
            std::mem::replace(&mut self.clipboard, InMemoryTextClipboard::with_text(""));
        self.clipboard = clipboard.with_external_copy_after_read(text, false);
        self
    }

    /// クリップボードへの書き込みを失敗させる
    ///
    /// # Returns
    /// * `Self` - 書き込み失敗を予約した `self`
    #[must_use]
    pub fn with_write_failure(mut self) -> Self {
        let clipboard =
            std::mem::replace(&mut self.clipboard, InMemoryTextClipboard::with_text(""));
        self.clipboard = clipboard.fail_on_write();
        self
    }

    /// 変換モードを設定する
    ///
    /// # Arguments
    /// * `mode` - 適用する変換モード (`RefineMode`)
    ///
    /// # Returns
    /// * `Self` - 変換モードを設定した `self`
    #[must_use]
    pub fn with_mode(self, mode: RefineMode) -> Self {
        self.set_mode(mode);
        self
    }

    /// 履歴の有効・無効を設定する
    ///
    /// # Arguments
    /// * `enabled` - 履歴を有効にする場合は `true`
    ///
    /// # Returns
    /// * `Self` - 履歴設定を反映した `self`
    #[must_use]
    pub fn with_history(self, enabled: bool) -> Self {
        self.state.with_config_mut(|c| c.history_enabled = enabled);
        self
    }

    /// 連続記録モードの有効・無効を設定する
    ///
    /// # Arguments
    /// * `enabled` - 連続記録を有効にする場合は `true`
    ///
    /// # Returns
    /// * `Self` - 連続記録設定を反映した `self`
    #[must_use]
    pub fn with_continuous_record(self, enabled: bool) -> Self {
        self.state
            .with_config_mut(|c| c.continuous_record_enabled = enabled);
        self
    }

    /// 監視方式を設定する
    ///
    /// # Arguments
    /// * `mode` - 監視方式 (`MonitorMode`)
    ///
    /// # Returns
    /// * `Self` - 監視方式を設定した `self`
    #[must_use]
    pub fn with_monitor_mode(self, mode: MonitorMode) -> Self {
        self.state.with_config_mut(|c| c.monitor_mode = mode);
        self
    }

    /// 監視時に適用する加工パイプラインを設定する
    ///
    /// # Arguments
    /// * `pipeline` - 監視時に適用する変換モード列
    ///
    /// # Returns
    /// * `Self` - パイプラインを設定した `self`
    #[must_use]
    pub fn with_pipeline(self, pipeline: Vec<RefineMode>) -> Self {
        self.state.with_config_mut(|c| c.pipeline = pipeline);
        self
    }

    /// 変換モードを変更する
    ///
    /// # Arguments
    /// * `mode` - 適用する変換モード (`RefineMode`)
    pub fn set_mode(&self, mode: RefineMode) {
        self.state.with_config_mut(|c| c.mode = mode);
    }

    /// クリップボード上のテキストを返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `&str` - インメモリクリップボードの現在内容
    pub fn clipboard_text(&self) -> &str {
        self.clipboard.text()
    }

    /// クリップボード上の HTML 形式を返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `Option<&str>` - HTML 断片。HTML 形式が無い場合は `None`
    pub fn clipboard_html(&self) -> Option<&str> {
        self.clipboard.html()
    }

    /// クリップボード上のテキストを直接置き換える (外部コピーを模倣)
    ///
    /// # Arguments
    /// * `text` - 置き換え後のクリップボード内容
    pub fn replace_clipboard(&mut self, text: impl Into<String>) {
        self.clipboard
            .set_text(text.into())
            .expect("クリップボードの置換に失敗");
    }

    /// クリップボードを、指定した本文と画像のコピーで置き換える (Excel のコピーを模倣)
    ///
    /// # Arguments
    /// * `text` - コピーの本文
    pub fn copy_excel(&mut self, text: impl Into<String>) {
        // 変更トークンを進めたうえで、本文と画像を同時に置く
        self.replace_clipboard(text);
        let clipboard =
            std::mem::replace(&mut self.clipboard, InMemoryTextClipboard::with_text(""));
        self.clipboard = clipboard.with_source_image(1, 1, vec![1, 2, 3, 255]);
    }

    /// インメモリクリップボードを新しい内容で初期化する
    ///
    /// # Arguments
    /// * `text` - 再初期化後のクリップボード内容
    pub fn reset_clipboard(&mut self, text: impl Into<String>) {
        self.clipboard = InMemoryTextClipboard::with_text(text);
    }

    /// 履歴件数を返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `usize` - 保持している履歴件数
    pub fn history_len(&self) -> usize {
        self.state.clipboard_session().history_len()
    }

    /// 連続記録件数を返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `usize` - 保持している連続記録件数
    pub fn continuous_record_len(&self) -> usize {
        self.state.clipboard_session().continuous_record_len()
    }

    /// 履歴エントリを先頭 (最新) から順に復号して返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `Vec<String>` - 復号済み履歴本文の一覧 (先頭が最新)
    pub fn history_entries(&self) -> Vec<String> {
        self.state.clipboard_session().history_texts()
    }

    /// 連続記録エントリを先頭 (キュー先頭) から順に復号して返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `Vec<String>` - 復号済み連続記録本文の一覧 (先頭がキュー先頭)
    pub fn continuous_record_entries(&self) -> Vec<String> {
        self.state.clipboard_session().continuous_record_texts()
    }

    /// 指定インデックスの履歴本文を返す
    ///
    /// # Arguments
    /// * `index` - 履歴ストア内のインデックス (0 が最新)
    ///
    /// # Returns
    /// * `Option<String>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn history_entry_text(&self, index: usize) -> Option<String> {
        self.state
            .clipboard_session()
            .get_history_entry(index)
            .map(|s| s.to_string())
    }

    /// 指定インデックスの連続記録本文を返す
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<String>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn continuous_record_entry_text(&self, index: usize) -> Option<String> {
        self.state
            .clipboard_session()
            .get_continuous_record_entry(index)
            .map(|s| s.to_string())
    }

    /// 直近の加工前テキストを取り出す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `Option<String>` - 取り消し可能な加工があれば `Some(元テキスト)`
    pub fn take_undo_source(&self) -> Option<String> {
        self.state
            .clipboard_session()
            .take_undo_source()
            .map(|s| s.to_string())
    }

    /// 直近の書き戻し指紋が指定テキストと一致するか判定する
    ///
    /// # Arguments
    /// * `text` - 比較対象のクリップボード本文
    ///
    /// # Returns
    /// * `bool` - 直近の書き戻しと一致する場合は `true`
    pub fn matches_last_written(&self, text: &str) -> bool {
        self.state
            .clipboard_session()
            .with_processed_state(|ps| ps.matches_last_written(text))
    }

    /// 設定を変更する
    ///
    /// # Arguments
    /// * `f` - `AppConfig` へ排他アクセスして変更を行うクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャ `f` の戻り値
    pub fn with_config_mut<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> R {
        self.state.with_config_mut(f)
    }

    /// 監視ループのクリップボード更新処理を実行する
    ///
    /// `event_driven` は OS イベント通知を受けたかどうかを表す。`false` のときは
    /// ポーリングで変更トークンの更新を確認した場合として扱う
    /// `MonitorMode::Event` 設定時のワーカー挙動と一致させる場合は
    /// [`Self::run_configured_monitor_update`] を使う
    ///
    /// # Arguments
    /// * `event_driven` - OS イベント通知を受けた場合は `true`
    ///
    /// # Returns
    /// * `bool` - クリップボードへ書き込んだ場合は `true`
    pub fn run_monitor_update(&mut self, event_driven: bool) -> bool {
        let snap = self.state.monitor_snapshot();
        self.state.sync_regex_settings(&mut self.refine_ctx.regex);
        if snap.continuous_record_enabled {
            observe_continuous_record(&mut self.clipboard, &self.state);
            return false;
        }
        handle_clipboard_update(
            &mut self.clipboard,
            &self.state,
            &snap,
            if event_driven {
                ClipboardTrigger::Event
            } else {
                ClipboardTrigger::PollingChanged
            },
            &self.refine_ctx,
        ) == ClipboardObserveResult::ClipboardWritten
    }

    /// 設定の `monitor_mode` に応じた `event_driven` フラグで監視更新を実行する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `bool` - クリップボードへ書き込んだ場合は `true`
    pub fn run_configured_monitor_update(&mut self) -> bool {
        let event_driven = self
            .state
            .with_config(|c| c.monitor_mode == MonitorMode::Event);
        self.run_monitor_update(event_driven)
    }

    /// ワーカーへ送られる `ProcessMode` コマンドを処理する
    ///
    /// # Arguments
    /// * `mode` - 適用する変換モード (`RefineMode`)
    pub fn process_mode(&mut self, mode: RefineMode) {
        self.refine_ctx.regex = self.state.with_config(|c| c.regex.clone());
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::ProcessMode(mode),
        );
    }

    /// ワーカーへ送られる `ProcessOneshotPipeline` コマンドを処理する
    ///
    /// # Arguments
    /// * `pipeline` - 適用する変換モード列
    /// * `ctx` - 加工コンテキスト
    ///
    /// # Returns
    /// * `OneshotProcessResult` - 加工結果
    pub fn process_oneshot_pipeline(
        &mut self,
        pipeline: Vec<RefineMode>,
        ctx: RefineContext,
    ) -> crate::ipc::OneshotProcessResult {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::ProcessOneshotPipeline {
                pipeline,
                refine_ctx: ctx,
                reply: reply_tx,
                cancelled: std::sync::Arc::default(),
            },
        );
        reply_rx.recv().expect("oneshot reply")
    }

    /// ワーカーへ送られる `SetText` コマンドを処理する
    ///
    /// # Arguments
    /// * `text` - クリップボードへ設定するテキスト
    pub fn set_text(&mut self, text: impl AsRef<str>) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::SetText(secret_from(text.as_ref().to_string())),
        );
    }

    /// ワーカーへ送られる `SetOcrText` コマンドを処理する
    ///
    /// # Arguments
    /// * `text` - OCR 結果としてクリップボードへ設定するテキスト
    pub fn set_ocr_text(&mut self, text: impl AsRef<str>) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::SetOcrText(secret_from(text.as_ref().to_string())),
        );
    }

    /// ワーカーへ送られる `Undo` コマンドを処理する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    pub fn undo(&mut self) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::Undo,
        );
    }

    /// ワーカーへ送られる `PasteContinuousRecordFront` コマンドを処理する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    pub fn discharge_continuous_record_on_paste(&mut self) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::PasteContinuousRecordFront,
        );
    }

    /// 連続記録キューから指定位置を取り出してクリップボードへセットする
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    pub fn discharge_continuous_record_at(&mut self, index: usize) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::DischargeContinuousRecordAt(index),
        );
    }

    /// 連続記録キュー末尾 (直近の記録) を取り消す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - 取り消す記録があった場合は `true`、キューが空の場合は `false`
    pub fn undo_last_continuous_record(&self) -> bool {
        self.state.undo_last_continuous_record()
    }

    /// ワーカーへ送られる `RegisterClipFromClipboard` コマンドを処理する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    pub fn register_clip_from_clipboard(&mut self) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::RegisterClipFromClipboard,
        );
    }

    /// ワーカーへ送られる `CopyRegisteredClip` コマンドを処理する
    ///
    /// # Arguments
    /// * `index` - 登録クリップのインデックス
    pub fn copy_registered_clip(&mut self, index: usize) {
        handle_command(
            &mut self.clipboard,
            &self.state,
            &mut self.refine_ctx,
            ClipboardCommand::CopyRegisteredClip(index),
        );
    }

    /// 登録クリップ件数を返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardHarness`
    ///
    /// # Returns
    /// * `usize` - 登録済みクリップの件数
    pub fn registered_clip_count(&self) -> usize {
        self.state.with_config(|c| c.clips.len())
    }

    /// 指定インデックスの登録クリップ本文を返す (テキスト登録のみ)
    ///
    /// # Arguments
    /// * `index` - 登録クリップのインデックス
    ///
    /// # Returns
    /// * `Option<String>` - テキスト登録の場合は `Some(本文)`、画像登録や範囲外は `None`
    pub fn registered_clip_text(&self, index: usize) -> Option<String> {
        self.state
            .with_config(|c| c.resolve_registered_clip(index))
            .and_then(|clip| match clip {
                crate::config::ResolvedClip::Text(text) => Some(text.to_string()),
                crate::config::ResolvedClip::Image { .. } => None,
            })
    }
}
