//! ワンショット IPC の要求・応答プロトコル

use std::io::{self, BufRead, Read};

use crate::config::RegexStep;
use crate::refiner::{ClipboardProcessError, ClipboardProcessOutcome, RefineContext, RefineMode};

use serde::{Deserialize, Serialize};

// ======================================================================
// 定数
// ======================================================================
/// IPC メッセージ本文の最大バイト数
pub(crate) const MAX_IPC_MESSAGE_BYTES: usize = 64 * 1024;

// ======================================================================
// 行の読み取り
// ======================================================================
/// 上限付きで 1 行読み取った結果
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BoundedLine {
    /// 何も読まずに EOF へ達した
    Eof,
    /// 改行を除いた本文
    Line(String),
    /// 本文が `MAX_IPC_MESSAGE_BYTES` を超えた (超過分は読み取らない)
    TooLarge,
}

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
///
/// CLI 側で解決済みの有効プリセットを常駐へ渡す。`steps` があれば全ステップを適用し、
/// 旧クライアントの単一ステップフィールド (`pattern` 等) は `steps` が無いときだけ使う
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OneshotRegexOverride {
    /// 有効化するプリセット名 (名前の先頭一致。ステップ列適用時は表示名としても使う)
    #[serde(
        default,
        rename = "presetName",
        skip_serializing_if = "Option::is_none"
    )]
    pub preset_name: Option<String>,
    /// 有効プリセットのステップ列 (CLI 側で解決済み)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<RegexStep>>,
    /// 正規表現パターン (旧ワイヤ。`steps` が無いときだけ使う)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// 置換文字列 (旧ワイヤ。`steps` が無いときだけ使う)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    /// 大文字小文字を無視するか (旧ワイヤ。`steps` が無いときだけ使う)
    #[serde(
        default,
        rename = "caseInsensitive",
        skip_serializing_if = "Option::is_none"
    )]
    pub case_insensitive: Option<bool>,
    /// 複数行モード (旧ワイヤ。`steps` が無いときだけ使う)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiline: Option<bool>,
}

impl OneshotRegexOverride {
    /// 加工コンテキストへ上書きを適用する
    ///
    /// `steps` があれば全ステップで有効プリセットを置き換える。無ければ旧フィールドで
    /// 1 ステップ上書きする
    ///
    /// # Arguments
    /// * `self` - 上書き内容
    /// * `ctx` - 適用先コンテキスト
    pub fn apply_to(&self, ctx: &mut RefineContext) {
        if let Some(steps) = &self.steps {
            ctx.regex
                .apply_oneshot_steps_override(self.preset_name.as_deref(), steps);
            return;
        }

        if let Some(name) = &self.preset_name {
            let _ = ctx.regex.select_preset_by_name(name);
        }

        let has_step_override = self.pattern.is_some()
            || self.replacement.is_some()
            || self.case_insensitive.is_some()
            || self.multiline.is_some();
        if has_step_override {
            ctx.regex.apply_oneshot_step_override(
                self.pattern.clone(),
                self.replacement.clone(),
                self.case_insensitive,
                self.multiline,
            );
        }
    }

    /// 加工コンテキストからワイヤ用の上書きを生成する
    ///
    /// 有効プリセットの全ステップを送る。旧単一ステップフィールドは埋めない
    ///
    /// # Arguments
    /// * `ctx` - 送信するコンテキスト
    ///
    /// # Returns
    /// * `Self` - プリセット名とステップ列を埋めた上書き
    pub fn from_context(ctx: &RefineContext) -> Self {
        Self {
            preset_name: ctx.regex.active_preset().map(|preset| preset.name.clone()),
            steps: Some(ctx.regex.active_steps().to_vec()),
            pattern: None,
            replacement: None,
            case_insensitive: None,
            multiline: None,
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

/// IPC メッセージを 1 行読み取る
///
/// 本文 `MAX_IPC_MESSAGE_BYTES` と改行 1 バイトを超えて読み進めないため、
/// 改行を送らない相手からの巨大な入力でもメモリを確保し続けない
///
/// # Arguments
/// * `reader` - 読み取り元
///
/// # Returns
/// * `io::Result<BoundedLine>` - 読み取り結果。UTF-8 として不正な場合は `InvalidData`
pub(crate) fn read_bounded_line<R: BufRead>(reader: &mut R) -> io::Result<BoundedLine> {
    let limit = MAX_IPC_MESSAGE_BYTES as u64 + 1;
    let mut buf = Vec::new();
    let read = reader.by_ref().take(limit).read_until(b'\n', &mut buf)?;
    if read == 0 {
        return Ok(BoundedLine::Eof);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
    } else if read as u64 == limit {
        return Ok(BoundedLine::TooLarge);
    }
    String::from_utf8(buf)
        .map(BoundedLine::Line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
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
    use std::io::Cursor;

    use super::*;
    use crate::config::{RegexPreset, RegexSettings, RegexStep};

    fn multi_step_regex_settings() -> RegexSettings {
        RegexSettings {
            active_preset_id: "date".into(),
            presets: vec![RegexPreset {
                id: "date".into(),
                name: "日付".into(),
                steps: vec![
                    RegexStep {
                        pattern: r"(\d{4})-(\d{2})-(\d{2})".into(),
                        replacement: "$1/$2/$3".into(),
                        ..RegexStep::default()
                    },
                    RegexStep {
                        pattern: r"\s+".into(),
                        replacement: " ".into(),
                        ..RegexStep::default()
                    },
                ],
            }],
        }
    }

    /// 要求 JSON を往復できるようにすること
    #[test]
    fn request_json_roundtrip() {
        let ctx = RefineContext::with_regex(RegexSettings::from_step(RegexStep {
            pattern: "a".into(),
            replacement: "b".into(),
            case_insensitive: true,
            multiline: false,
        }));
        let request = build_request(&[RefineMode::Trim, RefineMode::UrlDecode], &ctx, "req-1");
        let json = serde_json::to_string(&request).expect("serialize");
        let parsed = parse_request(&json).expect("parse");
        assert_eq!(parsed, request);
        assert!(json.contains("\"steps\""));
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

    /// 旧ワイヤの単一ステップ上書きがコンテキストへ適用されるようにすること
    #[test]
    fn regex_override_applies_legacy_single_step_to_context() {
        let override_settings = OneshotRegexOverride {
            preset_name: None,
            steps: None,
            pattern: Some("foo".into()),
            replacement: Some("bar".into()),
            case_insensitive: Some(true),
            multiline: Some(true),
        };
        let mut ctx = RefineContext::default();
        override_settings.apply_to(&mut ctx);
        let step = &ctx.regex.active_steps()[0];
        assert_eq!(step.pattern, "foo");
        assert_eq!(step.replacement, "bar");
        assert!(step.case_insensitive);
        assert!(step.multiline);
        assert_eq!(ctx.regex.active_steps().len(), 1);
    }

    /// コンテキストの有効プリセット全ステップをワイヤへ載せること
    #[test]
    fn from_context_preserves_all_active_steps() {
        let ctx = RefineContext::with_regex(multi_step_regex_settings());
        let override_settings = OneshotRegexOverride::from_context(&ctx);

        assert_eq!(override_settings.preset_name.as_deref(), Some("日付"));
        let steps = override_settings.steps.expect("steps");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].pattern, r"(\d{4})-(\d{2})-(\d{2})");
        assert_eq!(steps[0].replacement, "$1/$2/$3");
        assert_eq!(steps[1].pattern, r"\s+");
        assert_eq!(steps[1].replacement, " ");
        assert!(override_settings.pattern.is_none());
        assert!(override_settings.replacement.is_none());
    }

    /// ステップ列上書きがコンテキストの全ステップを維持すること
    #[test]
    fn regex_override_applies_all_steps_to_context() {
        let ctx = RefineContext::with_regex(multi_step_regex_settings());
        let request = build_request(&[RefineMode::RegexReplace], &ctx, "req-2");
        let (pipeline, applied) = request.into_pipeline_and_context();

        assert_eq!(pipeline, vec![RefineMode::RegexReplace]);
        assert_eq!(
            applied
                .regex
                .active_preset()
                .map(|preset| preset.name.as_str()),
            Some("日付")
        );
        assert_eq!(applied.regex.active_steps(), ctx.regex.active_steps());
    }

    /// `steps` があるときは旧単一ステップフィールドを無視すること
    #[test]
    fn regex_override_prefers_steps_over_legacy_fields() {
        let override_settings = OneshotRegexOverride {
            preset_name: Some("日付".into()),
            steps: Some(vec![
                RegexStep {
                    pattern: "a".into(),
                    replacement: "A".into(),
                    ..RegexStep::default()
                },
                RegexStep {
                    pattern: "b".into(),
                    replacement: "B".into(),
                    ..RegexStep::default()
                },
            ]),
            pattern: Some("ignored".into()),
            replacement: Some("ignored".into()),
            case_insensitive: Some(true),
            multiline: Some(true),
        };
        let mut ctx = RefineContext::default();
        override_settings.apply_to(&mut ctx);

        assert_eq!(ctx.regex.active_steps().len(), 2);
        assert_eq!(ctx.regex.active_steps()[0].pattern, "a");
        assert_eq!(ctx.regex.active_steps()[1].pattern, "b");
    }

    /// 旧 JSON (単一ステップ) を解釈できるようにすること
    #[test]
    fn parse_request_accepts_legacy_single_step_regex() {
        let json = r#"{"type":"process-clipboard","requestId":"req-3","pipeline":["RegexReplace"],"regex":{"pattern":"foo","replacement":"bar","caseInsensitive":true}}"#;
        let parsed = parse_request(json).expect("parse");
        let (_, ctx) = parsed.into_pipeline_and_context();
        let step = &ctx.regex.active_steps()[0];
        assert_eq!(step.pattern, "foo");
        assert_eq!(step.replacement, "bar");
        assert!(step.case_insensitive);
        assert_eq!(ctx.regex.active_steps().len(), 1);
    }

    /// 上限ちょうどの本文と改行を 1 行として読み取ること
    #[test]
    fn read_bounded_line_accepts_body_at_limit() {
        let body = "x".repeat(MAX_IPC_MESSAGE_BYTES);
        let mut reader = Cursor::new(format!("{body}\nnext\n"));
        assert_eq!(
            read_bounded_line(&mut reader).expect("read"),
            BoundedLine::Line(body)
        );
        // 次の行は改行の直後から読める
        assert_eq!(
            read_bounded_line(&mut reader).expect("read"),
            BoundedLine::Line("next".into())
        );
    }

    /// 上限を超える本文は上限 + 1 バイトまでで読み取りを打ち切ること
    #[test]
    fn read_bounded_line_stops_reading_oversized_body() {
        let mut reader = Cursor::new(vec![b'x'; MAX_IPC_MESSAGE_BYTES * 4]);
        assert_eq!(
            read_bounded_line(&mut reader).expect("read"),
            BoundedLine::TooLarge
        );
        // 超過分は消費しない
        assert_eq!(reader.position(), MAX_IPC_MESSAGE_BYTES as u64 + 1);
    }

    /// 改行で終わらない短い入力と空入力を区別すること
    #[test]
    fn read_bounded_line_handles_eof() {
        let mut reader = Cursor::new(b"tail".to_vec());
        assert_eq!(
            read_bounded_line(&mut reader).expect("read"),
            BoundedLine::Line("tail".into())
        );
        // 読み切った後は EOF
        assert_eq!(
            read_bounded_line(&mut reader).expect("read"),
            BoundedLine::Eof
        );
    }

    /// UTF-8 として不正な行は `InvalidData` とすること
    #[test]
    fn read_bounded_line_rejects_invalid_utf8() {
        let mut reader = Cursor::new(b"\xff\xfe\n".to_vec());
        let err = read_bounded_line(&mut reader).expect_err("不正な UTF-8");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
