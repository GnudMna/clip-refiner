use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::lock_ext::LockExt;

use crate::config::RegexSettings;
use crate::platform::update::{
    AvailableUpdate, UpdateApplyOutcome, UpdateCheckOutcome, UpdateCheckSource,
    UpdateCheckUiResponse,
};

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
    update_busy: AtomicBool,
    update_check_source: Mutex<Option<UpdateCheckSource>>,
    update_check_result: Mutex<Option<UpdateCheckOutcome>>,
    available_update: Mutex<Option<AvailableUpdate>>,
    update_apply_result: Mutex<Option<UpdateApplyOutcome>>,
}

impl SettingsPendingState {
    /// 空の適用待ちキューを生成する
    pub fn new() -> Self {
        Self {
            patch: Mutex::new(None),
            regex_preview: Mutex::new(None),
            update_busy: AtomicBool::new(false),
            update_check_source: Mutex::new(None),
            update_check_result: Mutex::new(None),
            available_update: Mutex::new(None),
            update_apply_result: Mutex::new(None),
        }
    }

    /// 適用待ちの部分更新 JSON キューへ排他アクセスする
    pub fn patch_mut(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.patch.lock_ignore_poison()
    }

    /// キュー済みの部分更新 JSON を取り出す
    pub fn take_patch(&self) -> Option<String> {
        self.patch.lock_ignore_poison().take()
    }

    /// 正規表現プレビュー要求を格納する
    pub fn set_regex_preview_request(&self, request: RegexPreviewRequest) {
        *self.regex_preview.lock_ignore_poison() = Some(request);
    }

    /// 処理待ちの正規表現プレビュー要求を取り出す
    pub fn take_regex_preview_request(&self) -> Option<RegexPreviewRequest> {
        self.regex_preview.lock_ignore_poison().take()
    }

    /// 更新操作を開始できるとき `true` を返しビジーにする
    ///
    /// # Returns
    /// * `bool` - 開始できた場合は `true`
    pub fn try_begin_update_op(&self) -> bool {
        self.update_busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// 更新操作のビジー状態を解除する
    pub fn end_update_op(&self) {
        self.update_busy.store(false, Ordering::SeqCst);
    }

    /// 更新確認の起動元を記録する
    ///
    /// # Arguments
    /// * `source` - 設定ウィンドウまたはトレイ
    pub fn set_update_check_source(&self, source: UpdateCheckSource) {
        *self.update_check_source.lock_ignore_poison() = Some(source);
    }

    /// 更新確認の起動元を取り出す
    ///
    /// # Returns
    /// * `Option<UpdateCheckSource>` - 記録されていれば起動元
    pub fn take_update_check_source(&self) -> Option<UpdateCheckSource> {
        self.update_check_source.lock_ignore_poison().take()
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
    /// # Returns
    /// * `Option<UpdateCheckUiResponse>` - 結果があれば UI 向けレスポンス
    pub fn take_update_check_ui_response(&self) -> Option<UpdateCheckUiResponse> {
        self.update_check_result
            .lock_ignore_poison()
            .take()
            .map(|outcome| UpdateCheckUiResponse::from(&outcome))
    }

    /// 適用待ちの更新アセットを取り出す
    ///
    /// # Returns
    /// * `Option<AvailableUpdate>` - 確認済みの更新があればその情報
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
    /// # Returns
    /// * `Option<UpdateApplyOutcome>` - 適用結果
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

    /// 更新操作のビジーフラグを排他的に立てられること
    #[test]
    fn try_begin_update_op_is_exclusive() {
        let pending = SettingsPendingState::new();
        assert!(pending.try_begin_update_op());
        assert!(!pending.try_begin_update_op());
        pending.end_update_op();
        assert!(pending.try_begin_update_op());
    }

    /// Available 結果が適用用スロットへ控えられること
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

    /// `UpToDate` / `Error` では適用用スロットがクリアされること
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

    /// 更新確認の起動元を取り出して消費できること
    #[test]
    fn update_check_source_round_trip() {
        let pending = SettingsPendingState::new();
        pending.set_update_check_source(UpdateCheckSource::Tray);
        assert_eq!(
            pending.take_update_check_source(),
            Some(UpdateCheckSource::Tray)
        );
        assert_eq!(pending.take_update_check_source(), None);
    }

    /// 更新適用結果を取り出して消費できること
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
