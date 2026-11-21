//! ワンショット名前付きパイプ IPC の統合テスト用ヘルパー

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::ipc::{
    OneshotIpcResponse, OneshotIpcServer, OneshotProcessResult, process_via_pipe,
    send_raw_line_to_pipe, wait_for_named_pipe,
};
use crate::refiner::{RefineContext, RefineMode};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

/// テスト用パイプ名の連番
static PIPE_NAME_SEQ: AtomicU64 = AtomicU64::new(0);

// ======================================================================
// パブリック関数
// ======================================================================
/// 一意なテスト用パイプ名を生成する
///
/// # Returns
/// * `String` - `\\.\pipe\...` 形式の名前
pub fn unique_oneshot_pipe_name() -> String {
    let seq = PIPE_NAME_SEQ.fetch_add(1, Ordering::Relaxed);
    format!(
        r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-test-{}-{}",
        std::process::id(),
        seq
    )
}

/// スタブワーカー向けにワンショット応答スレッドを起動する
///
/// # Arguments
/// * `rx` - ワーカーコマンド受信側
/// * `handler` - `ProcessOneshotPipeline` を結果へ変換するクロージャ
///
/// # Returns
/// * `JoinHandle<()>` - 応答スレッドのハンドル
pub fn spawn_oneshot_stub_responder(
    rx: mpsc::Receiver<ClipboardCommand>,
    handler: impl Fn(Vec<RefineMode>, RefineContext) -> OneshotProcessResult + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(cmd) = rx.recv() {
            if let ClipboardCommand::ProcessOneshotPipeline {
                pipeline,
                refine_ctx,
                reply,
            } = cmd
            {
                let _ = reply.send(handler(pipeline, refine_ctx));
            }
        }
    })
}

/// テスト用パイプ名で IPC サーバーを起動し、接続可能になるまで待つ
///
/// # Arguments
/// * `worker` - クリップボードワーカーへのハンドル
///
/// # Returns
/// * `(OneshotIpcServer, String)` - サーバーガードとパイプ名
pub fn start_oneshot_ipc_for_test(
    worker: Arc<ClipboardWorkerHandle>,
) -> (OneshotIpcServer, String) {
    let pipe_name = unique_oneshot_pipe_name();
    let server = OneshotIpcServer::start_named(worker, pipe_name.clone());
    assert!(
        wait_for_named_pipe(&pipe_name, Duration::from_secs(2)),
        "テスト用ワンショット IPC パイプが待受を開始しなかった: {pipe_name}"
    );
    (server, pipe_name)
}

/// 指定パイプへワンショット加工を委譲する
///
/// # Arguments
/// * `pipe_name` - 接続先パイプ名
/// * `pipeline` - 適用する加工モード列
/// * `ctx` - 加工コンテキスト
///
/// # Returns
/// * `Result<OneshotIpcResponse, String>` - 応答またはエラー文字列
///
/// # Errors
/// 接続・送受信・プロトコルエラーのいずれか
pub fn process_oneshot_via_pipe(
    pipe_name: &str,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<OneshotIpcResponse, String> {
    process_via_pipe(pipe_name, pipeline, ctx).map_err(|e| e.to_string())
}

/// 指定パイプへ生の 1 行を送り、応答を待つ
///
/// # Arguments
/// * `pipe_name` - 接続先パイプ名
/// * `line` - 改行を含まない JSON など
///
/// # Returns
/// * `Result<OneshotIpcResponse, String>` - 応答またはエラー文字列
///
/// # Errors
/// 接続・送受信・プロトコルエラーのいずれか
pub fn send_raw_oneshot_line(pipe_name: &str, line: &str) -> Result<OneshotIpcResponse, String> {
    send_raw_line_to_pipe(pipe_name, line).map_err(|e| e.to_string())
}

/// 名前付きパイプが接続可能になるまで待つ
///
/// # Arguments
/// * `pipe_name` - 接続先パイプ名
/// * `timeout` - 待機上限
///
/// # Returns
/// * `bool` - タイムアウト前に待受を検知できた場合は `true`
pub fn wait_for_oneshot_pipe(pipe_name: &str, timeout: Duration) -> bool {
    wait_for_named_pipe(pipe_name, timeout)
}
