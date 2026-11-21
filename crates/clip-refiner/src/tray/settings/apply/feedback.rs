use serde::Serialize;

// ======================================================================
// 適用フィードバック
// ======================================================================
/// 設定パッチ適用結果 (設定ウィンドウ UI へ返す)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsApplyFeedback {
    /// 適用に成功したかどうか
    pub ok: bool,
    /// 対象フィールド (`launchAtLogin` など)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// ユーザー向けメッセージ
    pub message: String,
    /// デスクトップ通知を別経路で既に表示済みかどうか
    #[serde(skip)]
    pub(crate) desktop_already_notified: bool,
}

impl SettingsApplyFeedback {
    /// 成功メッセージを生成する
    pub(crate) fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            field: None,
            message: message.into(),
            desktop_already_notified: false,
        }
    }

    /// エラーメッセージを生成する
    pub(crate) fn error(field: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            field: field.map(str::to_string),
            message: message.into(),
            desktop_already_notified: false,
        }
    }

    /// 対象フィールドを付与する
    ///
    /// # Arguments
    /// * `self` - 付与対象のフィードバック
    /// * `field` - 対象フィールド (`hotkeys` など)
    ///
    /// # Returns
    /// * `Self` - フィールド付きのフィードバック
    pub(crate) fn with_field(mut self, field: Option<&str>) -> Self {
        self.field = field.map(str::to_string);
        self
    }

    /// デスクトップ通知を別経路で既に表示済みであることを記録する
    pub(crate) fn with_desktop_already_notified(mut self) -> Self {
        self.desktop_already_notified = true;
        self
    }
}

/// 非表示ウィンドウ向けにデスクトップ通知で適用エラーを伝えるべきかどうか
pub(crate) fn should_notify_apply_error_via_desktop(
    window_visible: bool,
    feedback: &SettingsApplyFeedback,
) -> bool {
    !window_visible && !feedback.ok && !feedback.desktop_already_notified
}

/// 部分更新パッチの適用結果
pub(crate) struct SettingsPatchApplyResult {
    /// UI 再同期が必要な場合のスコープと `force` フラグ
    pub refresh: Option<(super::refresh::SettingsRefreshScope, bool)>,
    /// ユーザーへ通知するフィードバック
    pub feedback: Option<SettingsApplyFeedback>,
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 適用フィードバック JSON が camelCase となるようにすること
    #[test]
    fn apply_feedback_json_uses_camel_case() {
        let feedback = SettingsApplyFeedback::error(
            Some("launchAtLogin"),
            "ログイン時自動起動の設定に失敗しました",
        );
        let json: serde_json::Value =
            serde_json::to_value(&feedback).expect("フィードバック JSON のシリアライズ");
        assert_eq!(json["ok"], false);
        assert_eq!(json["field"], "launchAtLogin");
        assert_eq!(json["message"], "ログイン時自動起動の設定に失敗しました");
        assert!(json.get("desktopAlreadyNotified").is_none());
    }

    /// 非表示ウィンドウでは未通知の適用エラーをデスクトップ通知へ送るようにすること
    #[test]
    fn apply_feedback_hidden_error_should_use_desktop_fallback() {
        let feedback = SettingsApplyFeedback::error(Some("regex"), "パターンが無効です");
        assert!(should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// 非表示でもデスクトップ通知済みのエラーは重複通知しないようにすること
    #[test]
    fn apply_feedback_hidden_error_skips_desktop_when_already_notified() {
        let feedback = SettingsApplyFeedback::error(Some("launchAtLogin"), "失敗")
            .with_desktop_already_notified();
        assert!(!should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// 表示中のウィンドウではデスクトップ通知フォールバックを使わないようにすること
    #[test]
    fn apply_feedback_visible_window_skips_desktop_fallback() {
        let feedback = SettingsApplyFeedback::error(None, "エラー");
        assert!(!should_notify_apply_error_via_desktop(true, &feedback));
    }

    /// 非表示かつ成功時はデスクトップ通知しないようにすること
    #[test]
    fn apply_feedback_hidden_success_skips_desktop_fallback() {
        let feedback = SettingsApplyFeedback::ok("保存しました");
        assert!(!should_notify_apply_error_via_desktop(false, &feedback));
    }
}
