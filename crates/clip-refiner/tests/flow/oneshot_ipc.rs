//! ワンショット名前付きパイプ IPC の実パイプ往復テスト

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "テスト専用のため、ヘルパー内の失敗もそのまま panic させる"
)]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use clip_refiner::test_helpers::{
    OneshotIpcStatus, OneshotProcessResult, process_oneshot_via_pipe, send_raw_oneshot_line,
    set_worker_alive, spawn_oneshot_stub_responder, start_oneshot_ipc_for_test,
    start_oneshot_ipc_with_read_timeout_for_test, stub_worker, test_app_state_arc,
};
use clip_refiner_core::config::{RegexPreset, RegexSettings, RegexStep};
use clip_refiner_core::{RefineContext, RefineMode};

/// 実パイプ IPC 統合テストの直列化ロック
///
/// 名前付きパイプの作成・接続を並列に行うと待受検知が不安定になるため直列化する
static ONESHOT_IPC_FLOW_LOCK: Mutex<()> = Mutex::new(());

fn with_oneshot_ipc_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = ONESHOT_IPC_FLOW_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f()
}

/// 待受が空くのを待ってから、要求を送らない接続を開く
fn open_idle_connection(pipe_name: &str) -> File {
    assert!(
        clip_refiner::test_helpers::wait_for_oneshot_pipe(pipe_name, Duration::from_secs(2)),
        "パイプ待受が開始されなかった"
    );
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .expect("要求を送らない接続を開く")
}

// ======================================================================
// テスト
// ======================================================================

/// 実パイプ経由で加工成功応答を受け取れるようにすること
#[test]
fn oneshot_ipc_pipe_returns_processed() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |_, _| OneshotProcessResult::Processed);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let response =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Processed);
        assert_ne!(response.request_id.len(), 0);
        assert!(response.is_success());
    });
}

/// 実パイプ経由で変更なし応答を受け取れるようにすること
#[test]
fn oneshot_ipc_pipe_returns_unchanged() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |_, _| OneshotProcessResult::Unchanged);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let response =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Unchanged);
        assert!(response.is_success());
    });
}

/// 正規表現上書きがワーカーへ渡るようにすること
#[test]
fn oneshot_ipc_pipe_forwards_regex_override() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |pipeline, ctx| {
            assert_eq!(pipeline, vec![RefineMode::RegexReplace]);
            let step = &ctx.regex.active_steps()[0];
            assert_eq!(step.pattern, "(world)");
            assert_eq!(step.replacement, "ClipRefiner");
            assert!(step.case_insensitive);
            OneshotProcessResult::Processed
        });
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let mut ctx = RefineContext::default();
        ctx.regex = RegexSettings::from_step(RegexStep {
            pattern: "(world)".into(),
            replacement: "ClipRefiner".into(),
            case_insensitive: true,
            ..Default::default()
        });

        let response = process_oneshot_via_pipe(&pipe_name, &[RefineMode::RegexReplace], &ctx)
            .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Processed);
    });
}

/// 複数ステップの正規表現プリセットがワーカーへ渡るようにすること
#[test]
fn oneshot_ipc_pipe_forwards_multi_step_regex() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |pipeline, ctx| {
            assert_eq!(pipeline, vec![RefineMode::RegexReplace]);
            let steps = ctx.regex.active_steps();
            assert_eq!(steps.len(), 2);
            assert_eq!(steps[0].pattern, r"(\d{4})-(\d{2})-(\d{2})");
            assert_eq!(steps[0].replacement, "$1/$2/$3");
            assert_eq!(steps[1].pattern, r"\s+");
            assert_eq!(steps[1].replacement, " ");
            OneshotProcessResult::Processed
        });
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let mut ctx = RefineContext::default();
        ctx.regex = RegexSettings {
            active_preset_id: "date".into(),
            presets: vec![RegexPreset {
                id: "date".into(),
                name: "日付".into(),
                steps: vec![
                    RegexStep {
                        pattern: r"(\d{4})-(\d{2})-(\d{2})".into(),
                        replacement: "$1/$2/$3".into(),
                        ..Default::default()
                    },
                    RegexStep {
                        pattern: r"\s+".into(),
                        replacement: " ".into(),
                        ..Default::default()
                    },
                ],
            }],
        };

        let response = process_oneshot_via_pipe(&pipe_name, &[RefineMode::RegexReplace], &ctx)
            .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Processed);
    });
}

/// 空パイプラインはワーカーを呼ばずエラー応答になるようにすること
#[test]
fn oneshot_ipc_pipe_rejects_empty_pipeline() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        // 空パイプラインはサーバー側で拒否するため、応答スレッドは起動しない
        drop(rx);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let response =
            process_oneshot_via_pipe(&pipe_name, &[], &RefineContext::default()).expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Error);
        assert!(response.message.contains("空"));
        assert!(!response.is_success());
    });
}

/// 不正 JSON はエラー応答になるようにすること
#[test]
fn oneshot_ipc_pipe_rejects_invalid_json() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        drop(rx);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let response = send_raw_oneshot_line(&pipe_name, "{not-json").expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Error);
        assert!(response.message.contains("解釈"));
        assert!(!response.is_success());
    });
}

/// ワーカー停止中はエラー応答になるようにすること
#[test]
fn oneshot_ipc_pipe_reports_worker_stopped() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        drop(rx);
        set_worker_alive(&state, false);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let response =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Error);
        assert!(response.message.contains("停止"));
        assert!(!response.is_success());
    });
}

/// サーバーが破棄した過大要求ではクライアントが切断を検知すること
#[test]
fn oneshot_ipc_pipe_drops_oversized_request_without_response() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        drop(rx);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        // メッセージ上限 (256 KiB) をわずかに超える行
        // (改行はクライアントの writeln が付与する)
        let oversized = "x".repeat(256 * 1024 + 64);
        let err = send_raw_oneshot_line(&pipe_name, &oversized).expect_err("応答なし切断を期待");
        let message = err.to_string();

        assert!(
            message.contains("切断")
                || message.contains("通信")
                || message.contains("タイムアウト"),
            "unexpected error: {message}"
        );
    });
}

/// 連続して複数回の委譲ができるようにすること
#[test]
fn oneshot_ipc_pipe_handles_sequential_requests() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |pipeline, _| {
            if pipeline == vec![RefineMode::Trim] {
                OneshotProcessResult::Processed
            } else {
                OneshotProcessResult::Unchanged
            }
        });
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        let first =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("1 回目");

        // 次インスタンスの待受開始を待ってから 2 回目を送る
        assert!(
            clip_refiner::test_helpers::wait_for_oneshot_pipe(&pipe_name, Duration::from_secs(2)),
            "2 回目用のパイプ待受が開始されなかった"
        );

        let second = process_oneshot_via_pipe(
            &pipe_name,
            &[RefineMode::UrlDecode],
            &RefineContext::default(),
        )
        .expect("2 回目");

        assert_eq!(first.status, OneshotIpcStatus::Processed);
        assert_eq!(second.status, OneshotIpcStatus::Unchanged);
    });
}

/// 要求を送らない接続が残っていても別の委譲を処理できるようにすること
#[test]
fn oneshot_ipc_pipe_serves_while_other_connection_is_idle() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |_, _| OneshotProcessResult::Processed);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        // 先に接続したクライアントが要求を送らないまま接続を保持する
        let _idle = open_idle_connection(&pipe_name);

        let response =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Processed);
    });
}

/// 処理枠が埋まっても、枠が空けば次の委譲を処理できるようにすること
#[test]
fn oneshot_ipc_pipe_recovers_after_connection_limit() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |_, _| OneshotProcessResult::Processed);
        let (_server, pipe_name) = start_oneshot_ipc_for_test(worker);

        // 処理枠 (4) と枠待ちの 1 接続を、要求を送らない接続で埋める
        let mut idle: Vec<File> = (0..5).map(|_| open_idle_connection(&pipe_name)).collect();

        // 1 接続目を閉じると枠待ちの接続が枠に入り、2 接続目を閉じると委譲用の枠が空く
        drop(idle.drain(..2));

        let response =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Processed);
    });
}

/// 要求を送らない接続が処理枠を埋めたままでも、読み取り期限で枠が空くようにすること
#[test]
fn oneshot_ipc_pipe_frees_slots_from_idle_connections_after_read_timeout() {
    with_oneshot_ipc_lock(|| {
        let read_timeout = Duration::from_millis(200);
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        let _responder = spawn_oneshot_stub_responder(rx, |_, _| OneshotProcessResult::Processed);
        let (_server, pipe_name) =
            start_oneshot_ipc_with_read_timeout_for_test(worker, read_timeout);

        // 処理枠 (4) と枠待ちの 1 接続を、要求を送らない接続で埋めたまま閉じない
        let _idle: Vec<File> = (0..5).map(|_| open_idle_connection(&pipe_name)).collect();

        // 処理中の接続が期限で切断され、枠待ちの接続も枠に入ったあと期限で切断される
        thread::sleep(read_timeout * 3);

        assert!(
            clip_refiner::test_helpers::wait_for_oneshot_pipe(&pipe_name, Duration::from_secs(2)),
            "期限切れ後のパイプ待受が開始されなかった"
        );
        let response =
            process_oneshot_via_pipe(&pipe_name, &[RefineMode::Trim], &RefineContext::default())
                .expect("IPC 往復");

        assert_eq!(response.status, OneshotIpcStatus::Processed);
    });
}

/// 改行を送らずに止まった接続は、期限で切断されたことをクライアントが検知できること
#[test]
fn oneshot_ipc_pipe_disconnects_partial_request_after_read_timeout() {
    with_oneshot_ipc_lock(|| {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        drop(rx);
        let (_server, pipe_name) =
            start_oneshot_ipc_with_read_timeout_for_test(worker, Duration::from_millis(200));

        let mut stream = open_idle_connection(&pipe_name);
        stream
            .write_all(b"{\"type\":")
            .expect("途中までの要求を送る");
        stream.flush().expect("flush");

        // サーバーが切断すると応答を受け取らずに EOF / 切断エラーになる
        let mut buf = Vec::new();
        let read = stream.read_to_end(&mut buf);
        assert!(
            matches!(read, Ok(0)) || read.is_err(),
            "期限切れで応答なしに切断されること: {read:?}"
        );
    });
}
