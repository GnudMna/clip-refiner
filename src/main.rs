#![cfg_attr(windows, windows_subsystem = "windows")]

mod notification;
mod refiner;
mod tray;

use crate::tray::tray_app;

use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::Parser;
use single_instance::SingleInstance;

#[cfg(windows)]
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "クリップボードのテキストを加工するツール",
    long_about = "
クリップボードのテキストを加工（エンコード/デコード/トリム）するツール

使用方法:
  引数なし: システムトレイに常駐し、クリップボードを監視して自動加工
  --mode指定: クリップボードの内容を一度だけ加工"
)]
struct Args {
    /// 実行モードの指定
    #[arg(short = 'm', long = "mode", value_enum)]
    mode: Option<refiner::RefineMode>,
}

fn main() -> Result<()> {
    setup_console();

    let args = Args::parse();

    let _instance = ensure_single_instance()?;

    if let Some(mode) = args.mode {
        run_once(mode)?;
    } else {
        tray_app::run_loop()?;
    }

    Ok(())
}

/// Windowsの場合、親プロセスのコンソールをアタッチする
fn setup_console() {
    #[cfg(windows)]
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// 多重起動を防止し、インスタンスを保持する
fn ensure_single_instance() -> Result<SingleInstance> {
    let instance = SingleInstance::new("com.y_hirata.clip-refiner")
        .context("多重起動防止のインスタンス作成に失敗しました")?;

    if !instance.is_single() {
        let msg = "ClipRefinerは既に実行されています。";
        notification::error::show_error_notification("多重起動", msg);
        // 多重起動時は即座に終了するため、ErrではなくOkの扱いにしつつメッセージを表示
        std::process::exit(0);
    }

    Ok(instance)
}

/// クリップボードの内容を一度だけ加工して終了する
fn run_once(mode: refiner::RefineMode) -> Result<()> {
    let mut clipboard = Clipboard::new().context("クリップボードの初期化に失敗しました")?;
    refiner::process_clipboard(&mut clipboard, mode);
    Ok(())
}
