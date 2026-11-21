//! ワンショット加工 (IPC 委譲またはローカル実行)

use anyhow::{Context, Result};

use super::args::Args;
use crate::config::{self, AppConfig};
use crate::refiner::{
    self, ClipboardProcessError, ClipboardProcessOutcome, OwnedClipboard, RefineContext, RefineMode,
};
use crate::{log_error, log_info, log_warn};

// ======================================================================
// ワンショット実行
// ======================================================================
/// クリップボードの内容を一度だけ加工して終了する
///
/// 常駐プロセスがある場合は IPC で委譲し、監視のエコー再加工を避ける
/// 常駐が無い場合はローカルでクリップボードを直接加工する
///
/// # Arguments
/// * `args` - CLI 引数
///
/// # Returns
/// * `Result<()>` - 加工成功または変更なしの場合は `Ok(())`、失敗時は `Err` を返す
pub(super) fn run_once(args: &Args) -> Result<()> {
    let pipeline = resolve_oneshot_pipeline(args)?;
    log_info!("ワンショットモードで実行: {:?}", pipeline);
    let load_outcome = AppConfig::load();
    if let Some(ref fallback) = load_outcome.fallback {
        log_warn!("{}", fallback.user_message());
        eprintln!("警告: {}", fallback.user_message());
    }
    if !load_outcome.unknown_keys.is_empty() {
        let message = config::format_unknown_config_keys_message(&load_outcome.unknown_keys);
        log_warn!("{message}");
        eprintln!("警告: {message}");
    }
    if load_outcome.unsupported_version_kept {
        let message = config::format_unsupported_config_version_message();
        log_warn!("{message}");
        eprintln!("警告: {message}");
    }
    let config = load_outcome.config;
    let ctx = build_refinement_context(&config, args);

    if crate::ipc::is_resident_running() {
        match crate::ipc::process_via_resident(&pipeline, &ctx) {
            Ok(response) => return report_oneshot_ipc_response(&response),
            Err(err) => {
                // 常駐ありで IPC に失敗した場合はローカル実行しない (二重操作を避ける)
                log_error!("常駐プロセスへのワンショット委譲に失敗: {err}");
                eprintln!("エラー: {err}");
                return Err(err.into());
            }
        }
    }

    run_once_local(&pipeline, &ctx)
}

/// 常駐 IPC の応答を CLI 出力へ反映する
fn report_oneshot_ipc_response(response: &crate::ipc::OneshotIpcResponse) -> Result<()> {
    use crate::ipc::OneshotIpcStatus;

    match response.status {
        OneshotIpcStatus::Processed | OneshotIpcStatus::ImageProcessed => {
            log_info!("ワンショット処理が完了しました (常駐委譲)");
            eprintln!("{}", response.message);
            Ok(())
        }
        OneshotIpcStatus::Unchanged => {
            log_info!("テキストに変更はありませんでした (常駐委譲)");
            eprintln!("{}", response.message);
            Ok(())
        }
        OneshotIpcStatus::Error => {
            log_error!("ワンショット処理に失敗 (常駐委譲): {}", response.message);
            eprintln!("エラー: {}", response.message);
            if let Some(detail) = &response.detail {
                eprintln!("詳細: {detail}");
            }
            Err(anyhow::anyhow!(response.message.clone()))
        }
    }
}

/// 常駐が無い場合にローカルでワンショット加工する
fn run_once_local(pipeline: &[RefineMode], ctx: &RefineContext) -> Result<()> {
    let mut clipboard = OwnedClipboard::new().context("クリップボードの初期化に失敗しました")?;

    match refiner::process_clipboard_pipeline(&mut clipboard, pipeline, ctx) {
        Ok(
            ClipboardProcessOutcome::Processed(_) | ClipboardProcessOutcome::ImageProcessed { .. },
        ) => {
            log_info!("ワンショット処理が完了しました");
            eprintln!("加工が完了しました");
            Ok(())
        }
        Ok(ClipboardProcessOutcome::Unchanged) => {
            log_info!("テキストに変更はありませんでした");
            eprintln!("テキストに変更はありませんでした");
            Ok(())
        }
        Err(e) if e.is_skip() => {
            log_warn!("ワンショット処理をスキップ: {} ({:?})", e.user_message(), e);
            eprintln!("スキップ: {}", e.user_message());
            Err(anyhow::anyhow!(e.user_message().to_string()))
        }
        Err(e) => {
            log_error!("ワンショット処理に失敗: {} ({:?})", e.user_message(), e);
            eprintln!("エラー: {}", e.user_message());
            if let ClipboardProcessError::ReadFailed(detail)
            | ClipboardProcessError::WriteFailed(detail) = &e
            {
                eprintln!("詳細: {detail}");
            }
            Err(anyhow::anyhow!(e.user_message().to_string()))
        }
    }
}

/// 設定と CLI 引数から加工コンテキストを組み立てる
fn build_refinement_context(config: &AppConfig, args: &Args) -> RefineContext {
    let mut ctx = RefineContext::from_config(config);

    if let Some(name) = &args.regex_preset {
        let _ = ctx.regex.select_preset_by_name(name);
    }

    let has_step_override = args.regex_pattern.is_some()
        || args.regex_replacement.is_some()
        || args.regex_case_insensitive
        || args.regex_multiline;
    if has_step_override {
        ctx.regex.apply_oneshot_step_override(
            args.regex_pattern.clone(),
            args.regex_replacement.clone(),
            args.regex_case_insensitive.then_some(true),
            args.regex_multiline.then_some(true),
        );
    }

    ctx
}

/// ワンショット実行用の加工パイプラインを解決する
fn resolve_oneshot_pipeline(args: &Args) -> Result<Vec<RefineMode>> {
    if args.pipeline.is_empty() {
        args.mode
            .map(|mode| vec![mode])
            .ok_or_else(|| anyhow::anyhow!("--mode または --pipeline を指定してください"))
    } else {
        Ok(args.pipeline.clone())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::args::{Args, sample_args_for_test};
    use super::{build_refinement_context, report_oneshot_ipc_response, resolve_oneshot_pipeline};
    use crate::config::AppConfig;
    use crate::ipc::{OneshotIpcResponse, OneshotIpcStatus};
    use crate::refiner::RefineMode;

    /// `--mode` 指定時は単一モードのパイプラインになるようにすること
    #[test]
    fn resolve_oneshot_pipeline_from_mode() {
        let args = sample_args_for_test();
        let pipeline = resolve_oneshot_pipeline(&args).expect("pipeline");
        assert_eq!(pipeline, vec![RefineMode::Trim]);
    }

    /// `--pipeline` 指定時は複数モードを順に適用すること
    #[test]
    fn resolve_oneshot_pipeline_from_pipeline_flag() {
        let args = Args {
            pipeline: vec![RefineMode::UrlDecode, RefineMode::Trim],
            mode: Some(RefineMode::JsonFormat),
            ..sample_args_for_test()
        };
        let pipeline = resolve_oneshot_pipeline(&args).expect("pipeline");
        assert_eq!(pipeline, vec![RefineMode::UrlDecode, RefineMode::Trim]);
    }

    /// モード未指定時はエラーになるようにすること
    #[test]
    fn resolve_oneshot_pipeline_requires_mode_or_pipeline() {
        let args = Args {
            mode: None,
            pipeline: Vec::new(),
            ..sample_args_for_test()
        };
        assert!(resolve_oneshot_pipeline(&args).is_err());
    }

    /// CLI 引数が正規表現設定を上書きすること
    #[test]
    fn build_refinement_context_applies_regex_overrides() {
        let config = AppConfig::default();
        let args = Args {
            regex_pattern: Some("foo".to_string()),
            regex_replacement: Some("bar".to_string()),
            regex_case_insensitive: true,
            regex_multiline: true,
            ..sample_args_for_test()
        };

        let ctx = build_refinement_context(&config, &args);

        let step = &ctx.regex.active_steps()[0];
        assert_eq!(step.pattern, "foo");
        assert_eq!(step.replacement, "bar");
        assert!(step.case_insensitive);
        assert!(step.multiline);
    }

    /// `--regex-preset` でプリセット名を選択できること
    #[test]
    fn build_refinement_context_selects_regex_preset_by_name() {
        use crate::config::{RegexPreset, RegexSettings, RegexStep};

        let config = AppConfig {
            regex: RegexSettings {
                active_preset_id: "a".to_string(),
                presets: vec![
                    RegexPreset {
                        id: "a".to_string(),
                        name: "A".to_string(),
                        steps: vec![RegexStep {
                            pattern: "1".to_string(),
                            ..RegexStep::default()
                        }],
                    },
                    RegexPreset {
                        id: "b".to_string(),
                        name: "日付".to_string(),
                        steps: vec![RegexStep {
                            pattern: "2".to_string(),
                            ..RegexStep::default()
                        }],
                    },
                ],
            },
            ..AppConfig::default()
        };
        let args = Args {
            regex_preset: Some("日付".to_string()),
            ..sample_args_for_test()
        };

        let ctx = build_refinement_context(&config, &args);
        assert_eq!(ctx.regex.active_preset_id, "b");
        assert_eq!(ctx.regex.active_steps()[0].pattern, "2");
    }

    /// CLI 上書きが無いとき有効プリセットの全ステップを維持すること
    #[test]
    fn build_refinement_context_keeps_all_preset_steps() {
        use crate::config::{RegexPreset, RegexSettings, RegexStep};

        let config = AppConfig {
            regex: RegexSettings {
                active_preset_id: "date".to_string(),
                presets: vec![RegexPreset {
                    id: "date".to_string(),
                    name: "日付".to_string(),
                    steps: vec![
                        RegexStep {
                            pattern: "1".to_string(),
                            replacement: "a".to_string(),
                            ..RegexStep::default()
                        },
                        RegexStep {
                            pattern: "2".to_string(),
                            replacement: "b".to_string(),
                            ..RegexStep::default()
                        },
                    ],
                }],
            },
            ..AppConfig::default()
        };
        let ctx = build_refinement_context(&config, &sample_args_for_test());

        assert_eq!(ctx.regex.active_steps().len(), 2);
        assert_eq!(ctx.regex.active_steps()[0].pattern, "1");
        assert_eq!(ctx.regex.active_steps()[1].pattern, "2");
    }

    /// 加工成功の IPC 応答を `Ok` として扱うようにすること
    #[test]
    fn report_oneshot_ipc_response_ok_on_processed() {
        let response = OneshotIpcResponse {
            request_id: "req-1".to_string(),
            status: OneshotIpcStatus::Processed,
            message: "加工が完了しました".to_string(),
            detail: None,
        };
        assert!(report_oneshot_ipc_response(&response).is_ok());

        let image = OneshotIpcResponse {
            status: OneshotIpcStatus::ImageProcessed,
            message: "画像加工が完了しました".to_string(),
            ..response
        };
        assert!(report_oneshot_ipc_response(&image).is_ok());
    }

    /// 変更なしの IPC 応答を `Ok` として扱うようにすること
    #[test]
    fn report_oneshot_ipc_response_ok_on_unchanged() {
        let response = OneshotIpcResponse {
            request_id: "req-2".to_string(),
            status: OneshotIpcStatus::Unchanged,
            message: "テキストに変更はありませんでした".to_string(),
            detail: None,
        };
        assert!(report_oneshot_ipc_response(&response).is_ok());
    }

    /// エラーの IPC 応答を `Err` として扱うようにすること
    #[test]
    fn report_oneshot_ipc_response_err_on_error() {
        let response = OneshotIpcResponse {
            request_id: "req-3".to_string(),
            status: OneshotIpcStatus::Error,
            message: "加工に失敗しました".to_string(),
            detail: Some("read failed".to_string()),
        };
        let err = report_oneshot_ipc_response(&response).expect_err("Error は Err");
        assert!(err.to_string().contains("加工に失敗しました"));
    }
}
