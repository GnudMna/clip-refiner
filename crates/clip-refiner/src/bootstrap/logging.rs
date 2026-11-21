//! コンソールアタッチとロギング初期化

use crate::config;
use crate::logger;
use crate::{log_info, log_warn};

use anyhow::{Context, Result};
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt as ts_fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

// ======================================================================
// コンソール設定
// ======================================================================
/// Windows環境で親プロセスのコンソールをアタッチする
///
/// これにより、コンソール(`cmd.exe` や `PowerShell`)から `cargo run` などで起動した場合に、
/// アプリケーションからの標準出力がコンソール上に表示される
pub(super) fn setup_console() {
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
pub(super) fn setup_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
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
    if config::is_portable_mode() {
        log_info!(
            "ポータブルモード: 設定ディレクトリ = {}",
            config_dir.display()
        );
    } else {
        log_info!("設定ディレクトリ = {}", config_dir.display());
    }

    Ok(guard)
}
