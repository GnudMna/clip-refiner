use std::str::FromStr;

use crate::config::{self, AppConfig};
use crate::consts;
use crate::logger;
use crate::platform;
use crate::refiner::{
    self, ClipboardProcessError, ClipboardProcessOutcome, OwnedClipboard, RefineContext, RefineMode,
};
use crate::tray::{self, E2eWindow};
use crate::user_error::fatal_startup;
use crate::{log_error, log_info, log_warn};

use anyhow::{Context, Result};
use clap::Parser;
use single_instance::SingleInstance;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt as ts_fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションの起動処理
///
/// # Returns
/// * `Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
pub fn run() -> Result<()> {
    setup_console();

    let args = Args::parse();
    if args.e2e_window.is_some() {
        tray::e2e::activate();
    }

    let _guard = match setup_logging() {
        Ok(guard) => guard,
        Err(err) => {
            report_fatal_startup_error("ログの初期化", &err);
            return Err(err);
        }
    };

    platform::init_notifications();

    // 常駐起動のみ多重起動を防止する。ワンショットは常駐中でも実行できる
    let _instance = if is_oneshot(&args) {
        None
    } else {
        Some(match ensure_single_instance(args.e2e_window.is_some()) {
            Ok(instance) => instance,
            Err(err) => {
                report_fatal_startup_error("多重起動防止", &err);
                return Err(err);
            }
        })
    };

    if let Err(err) = run_application(&args) {
        log_error!("アプリケーションの実行に失敗: {:#}", err);
        report_fatal_startup_error("実行", &err);
        return Err(err);
    }

    Ok(())
}

/// 引数に応じて常駐またはワンショット実行を行う
fn run_application(args: &Args) -> Result<()> {
    if is_oneshot(args) {
        run_once(args)
    } else {
        tray::run_loop(args.e2e_window)
    }
}

/// ワンショット実行 (`--mode` / `--pipeline`) かどうかを判定する
fn is_oneshot(args: &Args) -> bool {
    args.mode.is_some() || !args.pipeline.is_empty()
}

// ======================================================================
// 引数設定
// ======================================================================
/// コマンドライン引数
///
/// アプリケーションの動作モード(常駐またはワンショット)を指定する
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "クリップボードのテキストを加工するツール",
    help_template = "\
{about-with-newline}
使用方法:
    引数なし: システムトレイに常駐し、クリップボードを監視して自動加工
    --mode / --pipeline: クリップボードの内容を一度だけ加工 (常駐中は IPC 委譲)

{all-args}
"
)]
struct Args {
    /// 実行モードの指定(ワンショット実行用)
    #[arg(short = 'm', long = "mode", value_parser = parse_refine_mode)]
    mode: Option<RefineMode>,
    /// ワンショットで順に適用する加工モード列 (`--mode` より優先)
    #[arg(long = "pipeline", value_parser = parse_refine_mode, num_args = 1..)]
    pipeline: Vec<RefineMode>,
    /// 正規表現パターン (`config.toml` の `regex.pattern` を上書き)
    #[arg(long = "regex-pattern")]
    regex_pattern: Option<String>,
    /// 正規表現の置換文字列 (`config.toml` の `regex.replacement` を上書き)
    #[arg(long = "regex-replacement")]
    regex_replacement: Option<String>,
    /// 正規表現で大文字小文字を無視する
    #[arg(long = "regex-case-insensitive")]
    regex_case_insensitive: bool,
    /// 正規表現で複数行モードを有効にする
    #[arg(long = "regex-multiline")]
    regex_multiline: bool,
    /// E2E 用: 指定 `WebView` を自動表示して常駐する (Playwright + CDP)
    #[arg(
        long = "e2e-window",
        value_enum,
        conflicts_with_all = ["mode", "pipeline"]
    )]
    e2e_window: Option<E2eWindow>,
}

// ======================================================================
// コンソール設定
// ======================================================================
/// Windows環境で親プロセスのコンソールをアタッチする
///
/// これにより、コンソール(`cmd.exe` や `PowerShell`)から `cargo run` などで起動した場合に、
/// アプリケーションからの標準出力がコンソール上に表示される
fn setup_console() {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

// ======================================================================
// ロギング初期化
// ======================================================================
/// ロギングシステムを初期化する
///
/// 設定ディレクトリ内の `logs` フォルダにローカル日付の日次ログファイルを作成し、
/// 標準出力とファイルの両方にログを出力するように設定する
/// また、システム全体のグローバルロガーもここで初期化される
///
/// # Returns
/// * `Result<tracing_appender::non_blocking::WorkerGuard>` - ログ出力非同期スレッドを維持するためのガード
fn setup_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let config_dir = config::get_config_dir()?;
    let log_dir = config_dir.join("logs");

    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).context("ログディレクトリの作成に失敗")?;
    }

    if let Err(e) = config::permissions::restrict_private_dir_permissions(&log_dir) {
        log_warn!("ログディレクトリのパーミッション設定に失敗: {:?}", e);
    }
    if let Err(e) = config::permissions::restrict_private_files_in_dir(&log_dir) {
        log_warn!("ログファイルのパーミッション設定に失敗: {:?}", e);
    }

    let file_appender = logger::LocalDailyFileAppender::new(&log_dir, "clip-refiner.log")?;
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = ts_fmt::layer().with_writer(non_blocking).with_ansi(false);

    // stdout への出力はデバッグビルド限定とする
    let builder = tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .with(file_layer);

    #[cfg(debug_assertions)]
    let builder = builder.with(ts_fmt::layer().with_writer(std::io::stdout));

    builder.init();

    logger::cleanup_on_startup(&log_dir);

    log_info!("アプリケーションを起動しました");

    Ok(guard)
}

// ======================================================================
// 多重起動防止
// ======================================================================
/// トレイ常駐の多重起動を防止し、インスタンスを保持する
///
/// ワンショット実行では呼ばない。通常の常駐起動では `APP_ID` を使う。
/// E2E 起動ではプロセス固有 ID にし、常駐インスタンスと衝突しない
///
/// # Arguments
/// * `e2e` - E2E モードなら `true`
///
/// # Returns
/// * `Result<SingleInstance>` - シングルインスタンスであることが確認できた場合、そのインスタンスを返す
///   既に他の常駐インスタンスが実行中の場合は、通知を表示してプロセスを直ちに終了する
fn ensure_single_instance(e2e: bool) -> Result<SingleInstance> {
    let instance_id = if e2e {
        format!("{}-e2e-{}", consts::APP_ID, std::process::id())
    } else {
        consts::APP_ID.to_string()
    };
    let instance = SingleInstance::new(&instance_id)
        .context("多重起動防止のインスタンス作成に失敗しました")?;

    if !instance.is_single() {
        let msg = format!("{}は既に実行されています。", consts::APP_NAME);
        log_warn!("{}", msg);
        platform::show_notification("多重起動", &msg);
        // 多重起動時は即座に終了するため、ErrではなくOkの扱いにしつつメッセージを表示
        std::process::exit(0);
    }

    Ok(instance)
}

// ======================================================================
// ワンショット実行
// ======================================================================
/// クリップボードの内容を一度だけ加工して終了する
///
/// 常駐プロセスがある場合は IPC で委譲し、監視のエコー再加工を避ける。
/// 常駐が無い場合はローカルでクリップボードを直接加工する
///
/// # Arguments
/// * `args` - CLI 引数
///
/// # Returns
/// * `Result<()>` - 加工成功または変更なしの場合は `Ok(())`、失敗時は `Err` を返す
fn run_once(args: &Args) -> Result<()> {
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
    let config = load_outcome.config;
    let ctx = build_refinement_context(&config, args);

    if crate::ipc::is_resident_running() {
        match crate::ipc::process_via_resident(&pipeline, &ctx) {
            Ok(response) => return report_oneshot_ipc_response(&response),
            Err(err) => {
                // 常駐ありで IPC に失敗した場合はローカル実行しない (二重操作を避ける)
                log_error!("常駐プロセスへのワンショット委譲に失敗: {err}");
                eprintln!("エラー: {err}");
                return Err(anyhow::anyhow!(err.to_string()));
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

    if let Some(pattern) = &args.regex_pattern {
        ctx.regex.pattern.clone_from(pattern);
    }
    if let Some(replacement) = &args.regex_replacement {
        ctx.regex.replacement.clone_from(replacement);
    }
    if args.regex_case_insensitive {
        ctx.regex.case_insensitive = true;
    }
    if args.regex_multiline {
        ctx.regex.multiline = true;
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

/// CLI 引数の加工モード名 (kebab-case) を `RefineMode` へ変換する
fn parse_refine_mode(value: &str) -> Result<RefineMode, String> {
    RefineMode::from_str(value).map_err(|_| format!("'{value}' は有効な加工モードではありません"))
}

// ======================================================================
// 起動失敗の通知
// ======================================================================
/// 起動失敗をログ・標準エラー・デスクトップ通知へ報告する
///
/// Windows の GUI ビルドではコンソールがないため、通知が主なフィードバックになる
fn report_fatal_startup_error(context: &str, err: &anyhow::Error) {
    let notification = fatal_startup(context, err);
    eprintln!("{}: {}", notification.title(), notification.body());
    platform::show_notification(notification.title(), notification.body());
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::AppConfig;
    use crate::refiner::RefineMode;

    fn sample_args() -> Args {
        Args {
            mode: Some(RefineMode::Trim),
            pipeline: Vec::new(),
            regex_pattern: None,
            regex_replacement: None,
            regex_case_insensitive: false,
            regex_multiline: false,
            e2e_window: None,
        }
    }

    /// `--mode` 指定時はワンショットと判定すること
    #[test]
    fn is_oneshot_when_mode_is_set() {
        assert!(is_oneshot(&sample_args()));
    }

    /// `--pipeline` 指定時はワンショットと判定すること
    #[test]
    fn is_oneshot_when_pipeline_is_set() {
        let args = Args {
            mode: None,
            pipeline: vec![RefineMode::Trim],
            ..sample_args()
        };
        assert!(is_oneshot(&args));
    }

    /// モード未指定時は常駐起動と判定すること
    #[test]
    fn is_oneshot_false_for_tray_launch() {
        let args = Args {
            mode: None,
            pipeline: Vec::new(),
            ..sample_args()
        };
        assert!(!is_oneshot(&args));
    }

    /// `--mode` 指定時は単一モードのパイプラインになるようにすること
    #[test]
    fn resolve_oneshot_pipeline_from_mode() {
        let args = sample_args();
        let pipeline = resolve_oneshot_pipeline(&args).expect("pipeline");
        assert_eq!(pipeline, vec![RefineMode::Trim]);
    }

    /// `--pipeline` 指定時は複数モードを順に適用すること
    #[test]
    fn resolve_oneshot_pipeline_from_pipeline_flag() {
        let args = Args {
            pipeline: vec![RefineMode::UrlDecode, RefineMode::Trim],
            mode: Some(RefineMode::JsonFormat),
            ..sample_args()
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
            ..sample_args()
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
            ..sample_args()
        };

        let ctx = build_refinement_context(&config, &args);

        assert_eq!(ctx.regex.pattern, "foo");
        assert_eq!(ctx.regex.replacement, "bar");
        assert!(ctx.regex.case_insensitive);
        assert!(ctx.regex.multiline);
    }
}
