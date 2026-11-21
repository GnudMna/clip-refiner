//! ログオンセッション単位のパイプ名解決と接続先の検証
//!
//! 多重起動防止の mutex はセッションごとの名前空間にあるが、名前付きパイプはマシン全体で 1 つの名前空間を共有する
//! そのため常駐パイプ名にセッション ID を付け、別セッションの常駐や先取りされたパイプへ接続しないようにする

use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::GetNamedPipeServerSessionId;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;

use crate::consts;

// ======================================================================
// パブリック関数
// ======================================================================
/// 現在のログオンセッションに対応する常駐パイプ名を返す
///
/// # Returns
/// * `io::Result<String>` - `\\.\pipe\...` 形式のパイプ名
///
/// # Errors
/// セッション ID を取得できない場合
pub(super) fn resident_pipe_name() -> io::Result<String> {
    Ok(pipe_name_for_session(current_session_id()?))
}

/// パイプのサーバー側が現在と同じログオンセッションにあるかを返す
///
/// # Arguments
/// * `pipe` - 接続済みのクライアント側パイプ
///
/// # Returns
/// * `io::Result<bool>` - 同じセッションなら `true`
///
/// # Errors
/// セッション ID を取得できない場合
pub(super) fn is_server_in_current_session(pipe: &File) -> io::Result<bool> {
    let mut server_session = 0u32;
    // SAFETY: `pipe` が所有する有効なパイプハンドルを借用し、`server_session` は呼び出し中に生存する
    unsafe { GetNamedPipeServerSessionId(HANDLE(pipe.as_raw_handle()), &raw mut server_session) }
        .map_err(io::Error::other)?;
    Ok(server_session == current_session_id()?)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 指定セッション ID の常駐パイプ名を組み立てる
fn pipe_name_for_session(session_id: u32) -> String {
    format!("{}.{session_id}", consts::ONESHOT_IPC_PIPE_NAME)
}

/// 現在のプロセスが属するログオンセッション ID を返す
fn current_session_id() -> io::Result<u32> {
    let mut session_id = 0u32;
    // SAFETY: `session_id` は呼び出し中に生存する
    unsafe { ProcessIdToSessionId(std::process::id(), &raw mut session_id) }
        .map_err(io::Error::other)?;
    Ok(session_id)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;

    use super::{is_server_in_current_session, pipe_name_for_session, resident_pipe_name};
    use crate::consts;
    use crate::ipc::server::create_pipe_instance_named;

    /// セッションごとに異なるパイプ名を組み立てること
    #[test]
    fn pipe_name_differs_per_session() {
        assert_eq!(
            pipe_name_for_session(2),
            format!(r"{}.2", consts::ONESHOT_IPC_PIPE_NAME)
        );
        assert_ne!(pipe_name_for_session(1), pipe_name_for_session(2));
    }

    /// 現在セッションの常駐パイプ名を解決できること
    #[test]
    fn resident_pipe_name_has_session_suffix() {
        let name = resident_pipe_name().expect("session id");
        let suffix = name
            .strip_prefix(&format!("{}.", consts::ONESHOT_IPC_PIPE_NAME))
            .expect("prefix");
        assert!(suffix.parse::<u32>().is_ok(), "suffix: {suffix}");
    }

    /// 同じプロセスが作成したパイプは同一セッションと判定すること
    #[test]
    fn same_process_pipe_is_in_current_session() {
        let pipe_name = format!(
            r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-session-test-{}",
            std::process::id()
        );
        let _server = create_pipe_instance_named(&pipe_name, true).expect("create pipe");
        let client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_name)
            .expect("open pipe");
        assert!(is_server_in_current_session(&client).expect("session check"));
    }
}
