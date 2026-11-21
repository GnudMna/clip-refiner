use serde::Serialize;

use crate::refiner::preview_regex_replace;
use crate::tray::state::RegexPreviewRequest;

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
        let preview = preview_regex_replace(&request.sample, &request.settings);
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
    use crate::refiner::REGEX_PREVIEW_SAMPLE;

    /// 有効なパターンでプレビュー応答が生成されるようにすること
    #[test]
    fn preview_response_contains_replace_result() {
        let response = RegexPreviewResponse::from_request(&RegexPreviewRequest {
            request_id: 7,
            settings: RegexSettings::single(r"\d+", "#"),
            sample: REGEX_PREVIEW_SAMPLE.to_string(),
        });
        assert!(response.valid);
        assert_eq!(response.request_id, 7);
        assert_eq!(response.result.as_deref(), Some("Hello world #"));
    }

    /// 任意サンプル文字列でプレビューできるようにすること
    #[test]
    fn preview_response_uses_request_sample() {
        let response = RegexPreviewResponse::from_request(&RegexPreviewRequest {
            request_id: 8,
            settings: RegexSettings::single(r"foo", "bar"),
            sample: "foo baz".to_string(),
        });
        assert!(response.valid);
        assert_eq!(response.result.as_deref(), Some("bar baz"));
    }
}
