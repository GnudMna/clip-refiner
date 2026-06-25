use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::Parser;
use single_instance::SingleInstance;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt as ts_fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{self, AppConfig};
use crate::consts;
use crate::logger;
use crate::platform;
use crate::refiner::{
    self, ClipboardProcessError, ClipboardProcessOutcome, RefineContext, RefineMode,
};
use crate::tray;
use crate::{log_error, log_info, log_warn};

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションの起動処理
///
/// # Returns
/// * `Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
pub fn run() -> Result<()> {
    setup_console();

    let _guard = setup_logging()?;

    let args = Args::parse();

    let _instance = ensure_single_instance()?;

    if let Some(mode) = args.mode {
        run_once(mode, &args)?;
    } else {
        tray::run_loop()?;
    }

    Ok(())
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
    --mode指定: クリップボードの内容を一度だけ加工

{all-args}
"
)]
struct Args {
    /// 実行モードの指定(ワンショット実行用)
    #[arg(short = 'm', long = "mode", value_enum)]
    mode: Option<RefineMode>,
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
}

// ======================================================================
// コンソール設定
// ======================================================================
/// Windows環境で親プロセスのコンソールをアタッチする
///
/// これにより、コンソール(`cmd.exe` や `PowerShell`)から `cargo run` などで起動した場合に、
/// アプリケーションからの標準出力がコンソール上に表示される
fn setup_console() {
    #[cfg(windows)]
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

    #[cfg(windows)]
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

// ======================================================================
// ロギング初期化
// ======================================================================
/// ロギングシステムを初期化する
///
/// 設定ディレクトリ内の `logs` フォルダに日次のログファイルを作成し、
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

    let file_appender = tracing_appender::rolling::daily(&log_dir, "clip-refiner.log");
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
/// アプリケーションの多重起動を防止し、インスタンスを保持する
///
/// `APP_ID` を使用してシステム全体で一意のインスタンスを確認する
///
/// # Returns
/// * `Result<SingleInstance>` - シングルインスタンスであることが確認できた場合、そのインスタンスを返す
///   既に他のインスタンスが実行中の場合は、通知を表示してプロセスを直ちに終了する
fn ensure_single_instance() -> Result<SingleInstance> {
    let instance = SingleInstance::new(consts::APP_ID)
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
/// 常駐せずに、現在のクリップボードのテキストを指定されたモードで加工し、
/// 結果をクリップボードに書き戻す
///
/// # Arguments
/// * `mode` - 適用する加工モード (`RefineMode`)
///
/// # Returns
/// * `Result<()>` - 加工成功または変更なしの場合は `Ok(())`、失敗時は `Err` を返す
fn run_once(mode: RefineMode, args: &Args) -> Result<()> {
    log_info!("ワンショットモードで実行: {:?}", mode);
    let config = AppConfig::load();
    let ctx = build_refinement_context(&config, args);
    let mut clipboard = Clipboard::new().context("クリップボードの初期化に失敗しました")?;

    match refiner::process_clipboard(&mut clipboard, mode, &ctx) {
        Ok(ClipboardProcessOutcome::Processed(_)) => {
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
