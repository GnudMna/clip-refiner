use crate::refiner::{REGEX_PREVIEW_SAMPLE, preview_regex_replace};
use crate::tray::state::RegexPreviewRequest;

use serde::Serialize;

// ======================================================================
// プレビュー応答
// ======================================================================
/// JavaScript へ返す正規表現プレビュー結果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegexPreviewResponse {
    /// クライアント側リクエスト ID
    pub request_id: u64,
    /// パターンがコンパイル可能かどうか
    pub valid: bool,
    /// 状態メッセージ
    pub message: String,
    /// 置換後テキスト
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

impl RegexPreviewResponse {
    /// プレビュー要求を評価して応答を生成する
    pub(crate) fn from_request(request: &RegexPreviewRequest) -> Self {
        let preview = preview_regex_replace(REGEX_PREVIEW_SAMPLE, &request.settings);
        Self {
            request_id: request.request_id,
            valid: preview.valid,
            message: preview.message,
            result: preview.result,
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::RegexSettings;

    /// 有効なパターンでプレビュー応答が生成されること
    #[test]
    fn preview_response_contains_replace_result() {
        let response = RegexPreviewResponse::from_request(&RegexPreviewRequest {
            request_id: 7,
            settings: RegexSettings {
                pattern: r"\d+".to_string(),
                replacement: "#".to_string(),
                ..RegexSettings::default()
            },
        });
        assert!(response.valid);
        assert_eq!(response.request_id, 7);
        assert_eq!(response.result.as_deref(), Some("Hello world #"));
    }
}
