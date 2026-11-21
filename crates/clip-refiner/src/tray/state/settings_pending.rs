use std::collections::VecDeque;
use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::app_event::ClipRef;
use super::lock_ext::LockExt;
use crate::config::RegexSettings;
use crate::platform::update::{
    AvailableUpdate, UpdateApplyOutcome, UpdateCheckOutcome, UpdateCheckUiResponse,
};
use crate::security::SecretString;

// ======================================================================
// 登録クリップ編集要求
// ======================================================================
/// 設定ウィンドウからの登録クリップ編集の保存要求
///
/// `AppEvent` は `Copy` かつ `Debug` のため、本文は含めずここへ預ける
pub(crate) struct ClipEditRequest {
    /// 編集対象の登録クリップ
    pub target: ClipRef,
    /// 新しい表示ラベル
    pub label: String,
    /// 新しい本文 (破棄時にゼロ化)
    pub text: SecretString,
}

impl fmt::Debug for ClipEditRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipEditRequest")
            .field("target", &self.target)
            .field("label", &self.label)
            .field("text", &"...")
            .finish()
    }
}

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
    /// プレビューに使うサンプル文字列 (永続化しない)
    pub sample: String,
}

// ======================================================================
// 設定ウィンドウ適用待ち
// ======================================================================
/// 設定ウィンドウから受信した未処理の更新要求
pub(crate) struct SettingsPendingState {
    patch: Mutex<Option<String>>,
    regex_preview: Mutex<Option<RegexPreviewRequest>>,
    clip_edits: Mutex<VecDeque<ClipEditRequest>>,
    update_busy: AtomicBool,
    update_check_result: Mutex<Option<UpdateCheckOutcome>>,
    available_update: Mutex<Option<AvailableUpdate>>,
    update_apply_result: Mutex<Option<UpdateApplyOutcome>>,
}

impl SettingsPendingState {
    /// 空の適用待ちキューを生成する
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    pub fn new() -> Self {
        Self {
            patch: Mutex::new(None),
            regex_preview: Mutex::new(None),
            clip_edits: Mutex::new(VecDeque::new()),
            update_busy: AtomicBool::new(false),
            update_check_result: Mutex::new(None),
            available_update: Mutex::new(None),
            update_apply_result: Mutex::new(None),
        }
    }

    /// 適用待ちの部分更新 JSON キューへ排他アクセスする
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `std::sync::MutexGuard<'_, Option<String>>` - 適用待ちの部分更新 JSON キューのロックガード
    pub fn patch_mut(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.patch.lock_ignore_poison()
    }

    /// キュー済みの部分更新 JSON を取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<String>` - 値がある場合は `Some`、ない場合は `None`
    pub fn take_patch(&self) -> Option<String> {
        self.patch.lock_ignore_poison().take()
    }

    /// 正規表現プレビュー要求を格納する
    ///
    /// # Arguments
    /// * `self` - 自身
    /// * `request` - リクエスト
    pub fn set_regex_preview_request(&self, request: RegexPreviewRequest) {
        *self.regex_preview.lock_ignore_poison() = Some(request);
    }

    /// 処理待ちの正規表現プレビュー要求を取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<RegexPreviewRequest>` - 値がある場合は `Some`、ない場合は `None`
    pub fn take_regex_preview_request(&self) -> Option<RegexPreviewRequest> {
        self.regex_preview.lock_ignore_poison().take()
    }

    /// 登録クリップの編集要求を末尾へ追加する
    ///
    /// # Arguments
    /// * `self` - 自身
    /// * `request` - 編集要求
    pub fn push_clip_edit(&self, request: ClipEditRequest) {
        self.clip_edits.lock_ignore_poison().push_back(request);
    }

    /// 処理待ちの登録クリップ編集要求を古い順に取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<ClipEditRequest>` - 値がある場合は `Some`、ない場合は `None`
    pub fn take_clip_edit(&self) -> Option<ClipEditRequest> {
        self.clip_edits.lock_ignore_poison().pop_front()
    }

    /// 更新操作を開始できるとき `true` を返しビジーにする
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - ビジーにできた場合は `true`、既に更新操作中の場合は `false`
    pub fn try_begin_update_op(&self) -> bool {
        self.update_busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// 更新操作のビジー状態を解除する
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn end_update_op(&self) {
        self.update_busy.store(false, Ordering::SeqCst);
    }

    /// 更新確認結果を格納する
    ///
    /// Available の場合は適用用スロットにも控え、それ以外では適用用スロットをクリアする
    ///
    /// # Arguments
    /// * `outcome` - 確認結果
    pub fn set_update_check_result(&self, outcome: UpdateCheckOutcome) {
        match &outcome {
            UpdateCheckOutcome::Available(update) => {
                *self.available_update.lock_ignore_poison() = Some(update.clone());
            }
            UpdateCheckOutcome::UpToDate { .. } | UpdateCheckOutcome::Error { .. } => {
                *self.available_update.lock_ignore_poison() = None;
            }
        }
        *self.update_check_result.lock_ignore_poison() = Some(outcome);
    }

    /// 更新確認結果を取り出し、UI レスポンスへ変換する
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<UpdateCheckUiResponse>` - 値がある場合は `Some`、ない場合は `None`
    pub fn take_update_check_ui_response(&self) -> Option<UpdateCheckUiResponse> {
        self.update_check_result
            .lock_ignore_poison()
            .take()
            .map(|outcome| UpdateCheckUiResponse::from(&outcome))
    }

    /// 適用待ちの更新アセットを取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<AvailableUpdate>` - 値がある場合は `Some`、ない場合は `None`
    pub fn take_available_update(&self) -> Option<AvailableUpdate> {
        self.available_update.lock_ignore_poison().take()
    }

    /// 更新適用結果を格納する
    ///
    /// # Arguments
    /// * `outcome` - 適用結果
    pub fn set_update_apply_result(&self, outcome: UpdateApplyOutcome) {
        *self.update_apply_result.lock_ignore_poison() = Some(outcome);
    }

    /// 更新適用結果を取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<UpdateApplyOutcome>` - 値がある場合は `Some`、ない場合は `None`
    pub fn take_update_apply_result(&self) -> Option<UpdateApplyOutcome> {
        self.update_apply_result.lock_ignore_poison().take()
    }
}

impl Default for SettingsPendingState {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_update() -> AvailableUpdate {
        AvailableUpdate {
            current_version: "0.13.0".to_string(),
            latest_version: "0.13.1".to_string(),
            tag: "v0.13.1".to_string(),
            asset_name: "clip-refiner-0.13.1-x86_64.msi".to_string(),
            size: 100,
            sha256: "a".repeat(64),
        }
    }

    /// 登録クリップの編集要求を送信順に取り出せ、`Debug` が本文を出力しないこと
    #[test]
    fn clip_edit_requests_are_fifo_and_debug_masks_text() {
        let pending = SettingsPendingState::new();
        for (index, text) in [(0, "secret-one"), (2, "secret-two")] {
            pending.push_clip_edit(ClipEditRequest {
                target: ClipRef::checked(index, 7),
                label: format!("label{index}"),
                text: crate::security::secret_from(text),
            });
        }

        let first = pending.take_clip_edit().expect("first");
        assert_eq!(
            (first.target, first.label.as_str()),
            (ClipRef::checked(0, 7), "label0")
        );
        assert_eq!(first.text.as_str(), "secret-one");
        let debug = format!("{first:?}");
        assert!(debug.contains("label0"));
        assert!(!debug.contains("secret-one"));
        assert_eq!(pending.take_clip_edit().expect("second").target.index, 2);
        assert!(pending.take_clip_edit().is_none());
    }

    /// 更新操作のビジーフラグを排他的に立てられるようにすること
    #[test]
    fn try_begin_update_op_is_exclusive() {
        let pending = SettingsPendingState::new();
        assert!(pending.try_begin_update_op());
        assert!(!pending.try_begin_update_op());
        pending.end_update_op();
        assert!(pending.try_begin_update_op());
    }

    /// Available 結果が適用用スロットへ控えられるようにすること
    #[test]
    fn available_check_result_stores_apply_target() {
        let pending = SettingsPendingState::new();
        pending.set_update_check_result(UpdateCheckOutcome::Available(sample_update()));

        let ui = pending.take_update_check_ui_response().expect("ui");
        assert!(matches!(ui, UpdateCheckUiResponse::Available { .. }));
        let update = pending.take_available_update().expect("available");
        assert_eq!(update.tag, "v0.13.1");
        assert!(pending.take_available_update().is_none());
    }

    /// `UpToDate` / `Error` では適用用スロットがクリアされるようにすること
    #[test]
    fn non_available_check_result_clears_apply_target() {
        let pending = SettingsPendingState::new();
        pending.set_update_check_result(UpdateCheckOutcome::Available(sample_update()));
        pending.set_update_check_result(UpdateCheckOutcome::UpToDate {
            current_version: "0.13.1".to_string(),
            latest_version: "0.13.1".to_string(),
        });
        assert!(pending.take_available_update().is_none());

        pending.set_update_check_result(UpdateCheckOutcome::Available(sample_update()));
        pending.set_update_check_result(UpdateCheckOutcome::Error {
            message: "失敗".to_string(),
        });
        assert!(pending.take_available_update().is_none());
    }

    /// 更新適用結果を取り出して消費できるようにすること
    #[test]
    fn update_apply_result_round_trip() {
        let pending = SettingsPendingState::new();
        pending.set_update_apply_result(UpdateApplyOutcome::MsiStarted);
        assert!(matches!(
            pending.take_update_apply_result(),
            Some(UpdateApplyOutcome::MsiStarted)
        ));
        assert!(pending.take_update_apply_result().is_none());
    }
}
