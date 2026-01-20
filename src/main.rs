#![cfg_attr(windows, windows_subsystem = "windows")]

mod coder;
mod notification;
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
    about = "クリップボードのテキストをパーセントエンコード/デコードするツール",
    long_about = "
クリップボードのテキストをパーセントエンコード/デコードするツール

使用方法:
  引数なし: システムトレイに常駐し、クリップボードを監視して自動変換
  --codec指定: クリップボードの内容を一度だけ変換"
)]
struct Args {
    /// コーデックの指定
    #[arg(short = 'c', long = "codec", value_enum)]
    codec: Option<coder::CodecMode>,
}

fn main() -> Result<()> {
    setup_console();

    let args = Args::parse();

    let _instance = ensure_single_instance()?;

    if let Some(codec) = args.codec {
        run_once(codec)?;
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
    let instance = SingleInstance::new("com.y_hirata.clip-coder")
        .context("多重起動防止のインスタンス作成に失敗しました")?;

    if !instance.is_single() {
        let msg = "ClipCoderは既に実行されています。";
        notification::error::show_error_notification("多重起動", msg);
        // 多重起動時は即座に終了するため、ErrではなくOkの扱いにしつつメッセージを表示
        std::process::exit(0);
    }

    Ok(instance)
}

/// クリップボードの内容を一度だけ変換して終了する
fn run_once(codec: coder::CodecMode) -> Result<()> {
    let mut clipboard = Clipboard::new().context("クリップボードの初期化に失敗しました")?;
    coder::process_clipboard(&mut clipboard, codec);
    Ok(())
}
