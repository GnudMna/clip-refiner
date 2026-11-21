//! ワンショット名前付きパイプ IPC の実パイプ往復テスト

use std::sync::Mutex;
use std::time::Duration;

use clip_refiner::test_helpers::{
    OneshotIpcStatus, OneshotProcessResult, process_oneshot_via_pipe, send_raw_oneshot_line,
    set_worker_alive, spawn_oneshot_stub_responder, start_oneshot_ipc_for_test, stub_worker,
    test_app_state_arc,
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
        assert!(!response.request_id.is_empty());
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

        // パイプバッファ (64 KiB) より小さく、メッセージ上限 (64 KiB) より大きい行
        // (改行はクライアントの writeln が付与する)
        let oversized = "x".repeat(64 * 1024 + 64);
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
