use std::sync::Mutex;

use super::lock_ext::LockExt;

use crate::config::RegexSettings;

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
}

// ======================================================================
// 設定ウィンドウ適用待ち
// ======================================================================
/// 設定ウィンドウから受信した未処理の更新要求
pub(crate) struct SettingsPendingState {
    patch: Mutex<Option<String>>,
    regex_preview: Mutex<Option<RegexPreviewRequest>>,
}

impl SettingsPendingState {
    /// 空の適用待ちキューを生成する
    pub fn new() -> Self {
        Self {
            patch: Mutex::new(None),
            regex_preview: Mutex::new(None),
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
}

impl Default for SettingsPendingState {
    fn default() -> Self {
        Self::new()
    }
}
