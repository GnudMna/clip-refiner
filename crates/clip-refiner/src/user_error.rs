//! アプリ層のユーザー向けエラー通知
//!
//! core の型付きエラーから通知タイトル・本文を生成し、ログとデスクトップ通知を一元化する

use crate::config::AddRegisteredClipError;
use crate::platform;
use crate::refiner::ClipboardProcessError;

// ======================================================================
// 通知
// ======================================================================
/// デスクトップ通知へ送るタイトルと本文
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserNotification {
    title: &'static str,
    body: String,
}

impl UserNotification {
    /// タイトルと本文から通知を組み立てる
    ///
    /// # Arguments
    /// * `title` - 通知タイトル
    /// * `body` - 通知本文
    ///
    /// # Returns
    /// * `Self` - 組み立てた `UserNotification`
    #[must_use]
    pub fn new(title: &'static str, body: impl Into<String>) -> Self {
        Self {
            title,
            body: body.into(),
        }
    }

    /// 静的な本文で通知を組み立てる
    ///
    /// # Arguments
    /// * `title` - 通知タイトル
    /// * `body` - 静的な通知本文
    ///
    /// # Returns
    /// * `Self` - 組み立てた `UserNotification`
    #[must_use]
    pub fn static_message(title: &'static str, body: &'static str) -> Self {
        Self {
            title,
            body: body.to_string(),
        }
    }

    /// 通知タイトルを返す
    ///
    /// # Arguments
    /// * `self` - この `UserNotification`
    ///
    /// # Returns
    /// * `&'static str` - 通知タイトル
    #[must_use]
    pub fn title(&self) -> &'static str {
        self.title
    }

    /// 通知本文を返す
    ///
    /// # Arguments
    /// * `self` - この `UserNotification`
    ///
    /// # Returns
    /// * `&str` - 通知本文
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// エラーをログ出力し、デスクトップ通知を表示する
    ///
    /// # Arguments
    /// * `self` - この `UserNotification`
    pub fn notify(&self) {
        crate::log_error!("{}: {}", self.title, self.body);
        platform::show_notification(self.title, &self.body);
    }

    /// 元エラーをログに含めて通知する
    ///
    /// # Arguments
    /// * `source` - ログ出力に含める元エラー
    pub fn notify_with_debug<E: std::fmt::Debug>(&self, source: &E) {
        crate::log_error!("{}: {} ({source:?})", self.title, self.body);
        platform::show_notification(self.title, &self.body);
    }
}

// ======================================================================
// クリップボード書き込み
// ======================================================================
/// クリップボード書き込み失敗の操作文脈
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardWriteContext {
    /// 履歴からの復元
    HistoryRestore,
    /// OCR 結果の書き込み
    Ocr,
    /// 加工の取り消し
    Undo,
    /// 連続記録からの取り出し
    ContinuousRecord,
    /// 登録クリップのコピー
    RegisteredClip,
}

impl ClipboardWriteContext {
    /// 文脈に応じたユーザー通知を返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardWriteContext`
    ///
    /// # Returns
    /// * `UserNotification` - 操作文脈に対応した通知
    #[must_use]
    pub fn notification(self) -> UserNotification {
        match self {
            Self::HistoryRestore => UserNotification::static_message(
                titles::CLIPBOARD,
                "履歴からの復元処理に失敗しました。",
            ),
            Self::Ocr => UserNotification::static_message(
                titles::OCR,
                "クリップボードへの書き込みに失敗しました",
            ),
            Self::Undo => {
                UserNotification::static_message(titles::CLIPBOARD, "加工の取り消しに失敗しました")
            }
            Self::ContinuousRecord => UserNotification::static_message(
                titles::CLIPBOARD,
                "連続記録からの取り出しに失敗しました",
            ),
            Self::RegisteredClip => UserNotification::static_message(
                titles::CLIPBOARD,
                "登録クリップのコピーに失敗しました。",
            ),
        }
    }
}

// ======================================================================
// core エラー変換
// ======================================================================
impl From<&ClipboardProcessError> for UserNotification {
    fn from(err: &ClipboardProcessError) -> Self {
        Self::new(titles::PROCESS, err.user_message())
    }
}

impl From<ClipboardProcessError> for UserNotification {
    fn from(err: ClipboardProcessError) -> Self {
        UserNotification::from(&err)
    }
}

/// クリップボードからの登録失敗向け通知を返す
///
/// # Arguments
/// * `err` - 登録クリップ追加エラー
///
/// # Returns
/// * `UserNotification` - 失敗理由に応じた通知
#[must_use]
pub fn registered_clip_register_failure(err: AddRegisteredClipError) -> UserNotification {
    let body = match err {
        AddRegisteredClipError::Empty => "クリップボードが空のため登録できません",
        AddRegisteredClipError::TooLarge => "テキストが長すぎるため登録できません",
        AddRegisteredClipError::LimitReached => "登録件数の上限に達しています",
        AddRegisteredClipError::ImageTooLarge => "画像が大きすぎるため登録できません",
        AddRegisteredClipError::ImageInvalid => "画像の形式が不正なため登録できません",
    };
    UserNotification::static_message(titles::REGISTERED_CLIP, body)
}

/// 設定ファイルの保存失敗をユーザー通知へ変換する
///
/// # Returns
/// * `UserNotification` - 保存失敗の案内
#[must_use]
pub fn config_save_failure() -> UserNotification {
    UserNotification::static_message(
        titles::SETTINGS,
        "設定の保存に失敗しました。変更は再起動後に失われる可能性があります",
    )
}

/// 設定パッチのキュー投入失敗をユーザー通知へ変換する
///
/// # Returns
/// * `UserNotification` - キュー投入失敗の案内
#[must_use]
pub fn settings_patch_enqueue_failure() -> UserNotification {
    UserNotification::static_message(
        titles::SETTINGS,
        "設定の更新キューへの反映に失敗しました。操作をやり直してください",
    )
}

/// 起動失敗向けの通知を組み立てる
///
/// # Arguments
/// * `context` - 失敗した初期化処理の文脈
/// * `err` - 起動失敗の原因
///
/// # Returns
/// * `UserNotification` - 長文を切り詰めた起動失敗通知
#[must_use]
pub fn fatal_startup(context: &str, err: &anyhow::Error) -> UserNotification {
    let body = format!("{context}: {err:#}");
    UserNotification::new(
        titles::STARTUP,
        truncate_notification_body(&body, NOTIFICATION_BODY_MAX_CHARS),
    )
}

/// 通知本文の最大文字数に合わせて切り詰める
///
/// # Arguments
/// * `body` - 切り詰め対象の通知本文
/// * `max_chars` - 保持する最大文字数
///
/// # Returns
/// * `String` - 上限超過時は末尾を `...` に置き換えた本文
#[must_use]
pub fn truncate_notification_body(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    format!("{}...", body.chars().take(keep).collect::<String>())
}

// ======================================================================
// 定数
// ======================================================================
/// 通知本文の既定最大文字数
const NOTIFICATION_BODY_MAX_CHARS: usize = 240;

/// 通知タイトル定数
pub mod titles {
    pub const CLIPBOARD: &str = "クリップボードエラー";
    pub const OCR: &str = "OCR エラー";
    pub const PROCESS: &str = "加工エラー";
    pub const REGISTERED_CLIP: &str = "登録クリップ";
    pub const SETTINGS: &str = "設定エラー";
    pub const STARTUP: &str = "起動エラー";
    pub const WORKER: &str = "クリップボードエラー";
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// クリップボード書き込み文脈ごとに異なる本文を返すようにすること
    #[test]
    fn clipboard_write_context_messages_differ() {
        let restore = ClipboardWriteContext::HistoryRestore.notification();
        let undo = ClipboardWriteContext::Undo.notification();
        assert_ne!(restore.body(), undo.body());
        assert_eq!(restore.title(), titles::CLIPBOARD);
    }

    /// 加工エラーは core の `user_message` を本文に使ようにすること
    #[test]
    fn clipboard_process_error_maps_user_message() {
        let notification = UserNotification::from(ClipboardProcessError::NoText);
        assert_eq!(notification.title(), titles::PROCESS);
        assert_eq!(notification.body(), "クリップボードにテキストがありません");
    }

    /// 設定保存失敗通知が固定の案内文を返すこと
    #[test]
    fn config_save_failure_has_settings_title() {
        let notification = config_save_failure();
        assert_eq!(notification.title(), titles::SETTINGS);
        assert!(notification.body().contains("保存に失敗"));
    }

    /// 設定パッチキュー失敗通知が固定の案内文を返すこと
    #[test]
    fn settings_patch_enqueue_failure_has_settings_title() {
        let notification = settings_patch_enqueue_failure();
        assert_eq!(notification.title(), titles::SETTINGS);
        assert!(notification.body().contains("キュー"));
    }

    /// 起動失敗通知は長文を切り詰めようにすること
    #[test]
    fn fatal_startup_truncates_long_body() {
        let err = anyhow::anyhow!("x".repeat(300));
        let notification = fatal_startup("初期化", &err);
        assert!(notification.body().chars().count() <= NOTIFICATION_BODY_MAX_CHARS);
        assert!(notification.body().ends_with("..."));
    }

    /// 登録クリップ登録失敗は文脈に応じた本文を返すようにすること
    #[test]
    fn registered_clip_register_failure_uses_contextual_message() {
        let notification = registered_clip_register_failure(AddRegisteredClipError::Empty);
        assert_eq!(
            notification.body(),
            "クリップボードが空のため登録できません"
        );
    }
}
