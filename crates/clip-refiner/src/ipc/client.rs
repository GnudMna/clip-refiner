//! ワンショット IPC クライアント (CLI 側)

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::protocol::{
    MAX_IPC_MESSAGE_BYTES, OneshotIpcRequest, OneshotIpcResponse, build_request, parse_response,
};

use crate::consts;
use crate::refiner::{RefineContext, RefineMode};

use single_instance::SingleInstance;

// ======================================================================
// 定数
// ======================================================================
/// 常駐パイプ接続のリトライ回数
const CONNECT_RETRY_COUNT: u32 = 8;
/// 接続リトライ間隔
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(75);

// ======================================================================
// エラー
// ======================================================================
/// ワンショット IPC クライアントの失敗理由
#[derive(Debug)]
pub enum ClientError {
    /// 常駐プロセスのパイプに接続できない
    Connect(String),
    /// 要求の送受信に失敗した
    Io(String),
    /// 応答 JSON が不正
    Protocol(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(detail) => {
                write!(f, "常駐プロセスへ接続できませんでした: {detail}")
            }
            Self::Io(detail) => {
                write!(f, "常駐プロセスとの通信に失敗しました: {detail}")
            }
            Self::Protocol(detail) => {
                write!(f, "常駐プロセスからの応答を解釈できませんでした: {detail}")
            }
        }
    }
}

impl std::error::Error for ClientError {}

impl ClientError {
    /// 接続失敗としてリトライ可能かどうか
    fn is_retryable_connect(&self) -> bool {
        matches!(self, Self::Connect(_))
    }
}

// ======================================================================
// パブリック関数
// ======================================================================
/// トレイ常駐インスタンスが起動中かどうか
///
/// 判定用に取得した `SingleInstance` は即座に破棄し、mutex を保持しない
///
/// # Returns
/// * `bool` - 常駐プロセスが既に存在する場合は `true`
pub fn is_resident_running() -> bool {
    match SingleInstance::new(consts::APP_ID) {
        Ok(instance) => !instance.is_single(),
        Err(_) => false,
    }
}

/// 常駐プロセスへワンショット加工を委譲する
///
/// # Arguments
/// * `pipeline` - 適用する加工モード列
/// * `ctx` - CLI 側で解決済みの加工コンテキスト
///
/// # Returns
/// * `Result<OneshotIpcResponse, ClientError>` - 常駐側の応答
///
/// # Errors
/// 接続・送受信・プロトコルエラーのいずれか
pub fn process_via_resident(
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<OneshotIpcResponse, ClientError> {
    let request_id = new_request_id();
    let request = build_request(pipeline, ctx, request_id);
    let mut last_error = None;

    for attempt in 0..CONNECT_RETRY_COUNT {
        match send_request(&request) {
            Ok(response) => return Ok(response),
            Err(err) if err.is_retryable_connect() && attempt + 1 < CONNECT_RETRY_COUNT => {
                last_error = Some(err);
                thread::sleep(CONNECT_RETRY_INTERVAL);
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_error.unwrap_or_else(|| ClientError::Connect("不明なエラー".into())))
}

// ======================================================================
// プライベート関数
// ======================================================================
fn new_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{}-{}", std::process::id(), nanos)
}

fn send_request(request: &OneshotIpcRequest) -> Result<OneshotIpcResponse, ClientError> {
    let mut stream = OpenOptions::new()
        .read(true)
        .write(true)
        .open(consts::ONESHOT_IPC_PIPE_NAME)
        .map_err(|e| ClientError::Connect(e.to_string()))?;

    let body = serde_json::to_string(request).map_err(|e| ClientError::Protocol(e.to_string()))?;
    if body.len() > MAX_IPC_MESSAGE_BYTES {
        return Err(ClientError::Protocol("要求が大きすぎます".into()));
    }

    writeln!(stream, "{body}").map_err(|e| ClientError::Io(e.to_string()))?;
    stream.flush().map_err(|e| ClientError::Io(e.to_string()))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let started = Instant::now();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| ClientError::Io(e.to_string()))?;
        if read == 0 {
            return Err(ClientError::Io("常駐プロセスが応答前に切断しました".into()));
        }
        if line.len() > MAX_IPC_MESSAGE_BYTES {
            return Err(ClientError::Protocol("応答が大きすぎます".into()));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if started.elapsed() > Duration::from_secs(30) {
                return Err(ClientError::Io("応答がタイムアウトしました".into()));
            }
            continue;
        }
        return parse_response(trimmed).map_err(|e| ClientError::Protocol(e.to_string()));
    }
}
