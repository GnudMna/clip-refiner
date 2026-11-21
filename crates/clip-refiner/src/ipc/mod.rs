//! ワンショット CLI と常駐プロセス間の IPC
//!
//! 常駐検出時は名前付きパイプ経由でクリップボード加工を委譲し、
//! 監視ループのエコー再加工を避ける

mod client;
mod protocol;
mod server;

pub use client::{is_resident_running, process_via_resident};
pub use protocol::{OneshotIpcResponse, OneshotIpcStatus, OneshotProcessResult};
pub use server::OneshotIpcServer;
