//! ワンショット IPC の要求・応答プロトコル

use crate::refiner::{ClipboardProcessError, ClipboardProcessOutcome, RefineContext, RefineMode};

use serde::{Deserialize, Serialize};

// ======================================================================
// 定数
// ======================================================================
/// IPC メッセージ本文の最大バイト数
pub(crate) const MAX_IPC_MESSAGE_BYTES: usize = 64 * 1024;

// ======================================================================
// ワーカー内部結果
// ======================================================================
/// クリップボードワーカーがワンショット加工後に返す結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OneshotProcessResult {
    /// テキストを加工して書き戻した
    Processed,
    /// 画像を書き込んだ
    ImageProcessed {
        /// 幅 (px)
        width: u32,
        /// 高さ (px)
        height: u32,
    },
    /// 変更がなかった
    Unchanged,
    /// 加工に失敗した
    Error {
        /// ユーザー向けメッセージ
        message: String,
        /// 詳細 (読み取り・書き込み失敗時)
        detail: Option<String>,
    },
}

impl OneshotProcessResult {
    /// クリップボード加工の成功結果から変換する
    ///
    /// # Arguments
    /// * `outcome` - 加工結果
    ///
    /// # Returns
    /// * `Self` - IPC / ワーカー向け結果
    pub fn from_outcome(outcome: &ClipboardProcessOutcome) -> Self {
        match outcome {
            ClipboardProcessOutcome::Processed(_) => Self::Processed,
            ClipboardProcessOutcome::ImageProcessed { width, height } => Self::ImageProcessed {
                width: *width,
                height: *height,
            },
            ClipboardProcessOutcome::Unchanged => Self::Unchanged,
        }
    }

    /// クリップボード加工の失敗から変換する
    ///
    /// # Arguments
    /// * `error` - 加工エラー
    ///
    /// # Returns
    /// * `Self` - エラー結果
    pub fn from_process_error(error: &ClipboardProcessError) -> Self {
        let detail = match error {
            ClipboardProcessError::ReadFailed(detail)
            | ClipboardProcessError::WriteFailed(detail) => Some(detail.clone()),
            _ => None,
        };
        Self::Error {
            message: error.user_message().to_string(),
            detail,
        }
    }

    /// ワイヤ応答へ変換する
    ///
    /// # Arguments
    /// * `self` - この結果
    /// * `request_id` - 対応する要求 ID
    ///
    /// # Returns
    /// * `OneshotIpcResponse` - JSON 応答
    pub fn into_response(self, request_id: String) -> OneshotIpcResponse {
        match self {
            Self::Processed => OneshotIpcResponse {
                request_id,
                status: OneshotIpcStatus::Processed,
                message: "加工が完了しました".to_string(),
                detail: None,
            },
            Self::ImageProcessed { width, height } => OneshotIpcResponse {
                request_id,
                status: OneshotIpcStatus::ImageProcessed,
                message: format!("画像加工が完了しました ({width}x{height})"),
                detail: None,
            },
            Self::Unchanged => OneshotIpcResponse {
                request_id,
                status: OneshotIpcStatus::Unchanged,
                message: "テキストに変更はありませんでした".to_string(),
                detail: None,
            },
            Self::Error { message, detail } => OneshotIpcResponse {
                request_id,
                status: OneshotIpcStatus::Error,
                message,
                detail,
            },
        }
    }
}

// ======================================================================
// ワイヤ型
// ======================================================================
/// ワンショット IPC 要求
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum OneshotIpcRequest {
    /// クリップボードを指定パイプラインで加工する
    ProcessClipboard {
        /// クライアント側リクエスト ID
        #[serde(rename = "requestId")]
        request_id: String,
        /// 適用する変換モード列
        pipeline: Vec<RefineMode>,
        /// 正規表現設定 (CLI 側で解決済み)
        #[serde(default)]
        regex: OneshotRegexOverride,
    },
}

/// ワンショット要求の正規表現上書き
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OneshotRegexOverride {
    /// 正規表現パターン
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// 置換文字列
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    /// 大文字小文字を無視するか
    #[serde(
        default,
        rename = "caseInsensitive",
        skip_serializing_if = "Option::is_none"
    )]
    pub case_insensitive: Option<bool>,
    /// 複数行モード
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiline: Option<bool>,
}

impl OneshotRegexOverride {
    /// 加工コンテキストへ上書きを適用する
    ///
    /// # Arguments
    /// * `self` - 上書き内容
    /// * `ctx` - 適用先コンテキスト
    pub fn apply_to(&self, ctx: &mut RefineContext) {
        if let Some(pattern) = &self.pattern {
            ctx.regex.pattern.clone_from(pattern);
        }
        if let Some(replacement) = &self.replacement {
            ctx.regex.replacement.clone_from(replacement);
        }
        if let Some(case_insensitive) = self.case_insensitive {
            ctx.regex.case_insensitive = case_insensitive;
        }
        if let Some(multiline) = self.multiline {
            ctx.regex.multiline = multiline;
        }
    }

    /// 加工コンテキストからワイヤ用の上書きを生成する
    ///
    /// # Arguments
    /// * `ctx` - 送信するコンテキスト
    ///
    /// # Returns
    /// * `Self` - 正規表現フィールドをすべて埋めた上書き
    pub fn from_context(ctx: &RefineContext) -> Self {
        Self {
            pattern: Some(ctx.regex.pattern.clone()),
            replacement: Some(ctx.regex.replacement.clone()),
            case_insensitive: Some(ctx.regex.case_insensitive),
            multiline: Some(ctx.regex.multiline),
        }
    }
}

/// ワンショット IPC 応答の状態
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OneshotIpcStatus {
    /// テキスト加工成功
    Processed,
    /// 画像加工成功
    ImageProcessed,
    /// 変更なし
    Unchanged,
    /// 失敗
    Error,
}

/// ワンショット IPC 応答
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OneshotIpcResponse {
    /// 対応する要求 ID
    #[serde(rename = "requestId")]
    pub request_id: String,
    /// 結果状態
    pub status: OneshotIpcStatus,
    /// ユーザー向けメッセージ
    pub message: String,
    /// 詳細 (任意)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl OneshotIpcResponse {
    /// ワーカー停止などサーバー側エラー応答を生成する
    ///
    /// # Arguments
    /// * `request_id` - 要求 ID
    /// * `message` - ユーザー向けメッセージ
    ///
    /// # Returns
    /// * `Self` - エラー応答
    pub fn error(request_id: String, message: impl Into<String>) -> Self {
        Self {
            request_id,
            status: OneshotIpcStatus::Error,
            message: message.into(),
            detail: None,
        }
    }

    /// CLI 終了コード用に成功かどうか判定する
    ///
    /// # Arguments
    /// * `self` - この応答
    ///
    /// # Returns
    /// * `bool` - `Error` 以外なら `true`
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn is_success(&self) -> bool {
        !matches!(self.status, OneshotIpcStatus::Error)
    }
}

/// パイプラインとコンテキストから IPC 要求を組み立てる
///
/// # Arguments
/// * `pipeline` - 適用する変換モード列
/// * `ctx` - CLI 側で解決済みの加工コンテキスト
/// * `request_id` - リクエスト ID
///
/// # Returns
/// * `OneshotIpcRequest` - 送信する要求
pub fn build_request(
    pipeline: &[RefineMode],
    ctx: &RefineContext,
    request_id: impl Into<String>,
) -> OneshotIpcRequest {
    OneshotIpcRequest::ProcessClipboard {
        request_id: request_id.into(),
        pipeline: pipeline.to_vec(),
        regex: OneshotRegexOverride::from_context(ctx),
    }
}

/// 要求 JSON をパースする
///
/// # Arguments
/// * `json` - JSON 文字列
///
/// # Returns
/// * `Result<OneshotIpcRequest, serde_json::Error>` - パース結果
pub fn parse_request(json: &str) -> Result<OneshotIpcRequest, serde_json::Error> {
    serde_json::from_str(json)
}

/// 応答 JSON をパースする
///
/// # Arguments
/// * `json` - JSON 文字列
///
/// # Returns
/// * `Result<OneshotIpcResponse, serde_json::Error>` - パース結果
pub fn parse_response(json: &str) -> Result<OneshotIpcResponse, serde_json::Error> {
    serde_json::from_str(json)
}

impl OneshotIpcRequest {
    /// 要求 ID を返す
    ///
    /// # Arguments
    /// * `self` - この要求
    ///
    /// # Returns
    /// * `&str` - リクエスト ID
    pub fn request_id(&self) -> &str {
        match self {
            Self::ProcessClipboard { request_id, .. } => request_id,
        }
    }

    /// パイプラインと加工コンテキストへ展開する
    ///
    /// # Arguments
    /// * `self` - この要求
    ///
    /// # Returns
    /// * `(Vec<RefineMode>, RefineContext)` - パイプラインとコンテキスト
    pub fn into_pipeline_and_context(self) -> (Vec<RefineMode>, RefineContext) {
        match self {
            Self::ProcessClipboard {
                pipeline, regex, ..
            } => {
                let mut ctx = RefineContext::default();
                regex.apply_to(&mut ctx);
                (pipeline, ctx)
            }
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 要求 JSON を往復できるようにすること
    #[test]
    fn request_json_roundtrip() {
        let ctx = RefineContext::with_regex(crate::config::RegexSettings {
            pattern: "a".into(),
            replacement: "b".into(),
            case_insensitive: true,
            multiline: false,
        });
        let request = build_request(&[RefineMode::Trim, RefineMode::UrlDecode], &ctx, "req-1");
        let json = serde_json::to_string(&request).expect("serialize");
        let parsed = parse_request(&json).expect("parse");
        assert_eq!(parsed, request);
    }

    /// 応答 JSON を往復できるようにすること
    #[test]
    fn response_json_roundtrip() {
        let response = OneshotProcessResult::Processed.into_response("req-1".into());
        let json = serde_json::to_string(&response).expect("serialize");
        let parsed = parse_response(&json).expect("parse");
        assert_eq!(parsed, response);
        assert!(parsed.is_success());
    }

    /// 正規表現上書きがコンテキストへ適用されること
    #[test]
    fn regex_override_applies_to_context() {
        let override_settings = OneshotRegexOverride {
            pattern: Some("foo".into()),
            replacement: Some("bar".into()),
            case_insensitive: Some(true),
            multiline: Some(true),
        };
        let mut ctx = RefineContext::default();
        override_settings.apply_to(&mut ctx);
        assert_eq!(ctx.regex.pattern, "foo");
        assert_eq!(ctx.regex.replacement, "bar");
        assert!(ctx.regex.case_insensitive);
        assert!(ctx.regex.multiline);
    }
}
