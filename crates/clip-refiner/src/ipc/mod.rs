//! ワンショット CLI と常駐プロセス間の IPC
//!
//! 常駐検出時は名前付きパイプ経由でクリップボード加工を委譲し、
//! 監視ループのエコー再加工を避ける
//! パイプは現在ユーザー専用 DACL と `PIPE_REJECT_REMOTE_CLIENTS` で保護する

mod client;
mod protocol;
mod server;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use client::ClientError;
pub use client::{is_resident_running, process_via_resident};
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use client::{process_via_pipe, send_raw_line_to_pipe, wait_for_named_pipe};
pub use protocol::{OneshotIpcResponse, OneshotIpcStatus, OneshotProcessResult};
pub use server::OneshotIpcServer;
