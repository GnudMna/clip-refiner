use std::collections::VecDeque;
use std::fmt;
use std::sync::Mutex;

use super::app_event::ClipRef;
use super::lock_ext::LockExt;
use crate::config::RegexSettings;
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
}
