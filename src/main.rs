#![cfg_attr(windows, windows_subsystem = "windows")] // Windowsでコンソールを出さないための設定

mod config;
mod consts;
mod logger;
mod notification;
mod refiner;
mod tray;

use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::Parser;
use single_instance::SingleInstance;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(windows)]
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

/// コマンドライン引数
///
/// アプリケーションの動作モード（常駐またはワンショット）を指定します。
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
    /// 実行モードの指定（ワンショット実行用）
    #[arg(short = 'm', long = "mode", value_enum)]
    mode: Option<refiner::RefineMode>,
}

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションのエントリポイント
///
/// 設定の初期化、ロギングのセットアップ、コマンドライン引数の解析を行い、
/// 常駐モードまたはワンショットモードで処理を開始します。
///
/// # Returns
/// * `Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返します。
fn main() -> Result<()> {
    setup_console();

    let _guard = setup_logging()?;

    let args = Args::parse();

    let _instance = ensure_single_instance()?;

    if let Some(mode) = args.mode {
        run_once(mode)?;
    } else {
        tray::run_loop()?;
    }

    Ok(())
}

// ======================================================================
// コンソール設定
// ======================================================================
/// Windows環境で親プロセスのコンソールをアタッチする
///
/// これにより、コンソール（`cmd.exe` や `PowerShell`）から `cargo run` などで起動した場合に、
/// アプリケーションからの標準出力がコンソール上に表示されるようになります。
fn setup_console() {
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
/// 標準出力とファイルの両方にログを出力するように設定します。
/// また、システム全体のグローバルロガーもここで初期化されます。
///
/// # Returns
/// * `Result<tracing_appender::non_blocking::WorkerGuard>` - ログ出力非同期スレッドを維持するためのガード。
fn setup_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let config_dir = config::get_config_dir()?;
    let log_dir = config_dir.join("logs");

    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).context("ログディレクトリの作成に失敗")?;
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "clip-refiner.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // stdout への出力はデバッグビルド限定とする。
    #[cfg(debug_assertions)]
    let stdout_layer = Some(tracing_subscriber::fmt::layer().with_writer(std::io::stdout));
    #[cfg(not(debug_assertions))]
    let stdout_layer: Option<tracing_subscriber::fmt::Layer<_, _, _, _>> = None;

    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .with(file_layer)
        .with(stdout_layer)
        .init();

    // グローバルロガーを初期化
    logger::init_global_logger(Box::new(logger::TracingLogger::new(log_dir)));

    log_info!("アプリケーションを起動しました");

    Ok(guard)
}

// ======================================================================
// 多重起動防止
// ======================================================================
/// アプリケーションの多重起動を防止し、インスタンスを保持する
///
/// `APP_ID` を使用してシステム全体で一意のインスタンスを確認します。
///
/// # Returns
/// * `Result<SingleInstance>` - シングルインスタンスであることが確認できた場合、そのインスタンスを返します。
///   既に他のインスタンスが実行中の場合は、通知を表示してプロセスを直ちに終了します。
fn ensure_single_instance() -> Result<SingleInstance> {
    let instance = SingleInstance::new(consts::APP_ID)
        .context("多重起動防止のインスタンス作成に失敗しました")?;

    if !instance.is_single() {
        let msg = format!("{}は既に実行されています。", consts::APP_NAME);
        log_warn!("{}", msg);
        notification::show_notification("多重起動", &msg);
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
/// 結果をクリップボードに書き戻します。
///
/// # Arguments
/// * `mode` - 適用する加工モード (`refiner::RefineMode`)
///
/// # Returns
/// * `Result<()>` - 処理が正常に完了した場合は `Ok(())` を返します。
fn run_once(mode: refiner::RefineMode) -> Result<()> {
    log_info!("ワンショットモードで実行: {:?}", mode);
    let mut clipboard = Clipboard::new().context("クリップボードの初期化に失敗しました")?;
    refiner::process_clipboard(&mut clipboard, mode);
    log_info!("ワンショット処理が完了しました");
    Ok(())
}
