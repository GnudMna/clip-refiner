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
    // Windowsの場合、親プロセスのコンソールをアタッチする
    #[cfg(windows)]
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let args = Args::parse();

    // 多重起動防止
    let instance = SingleInstance::new("com.y_hirata.clip-coder")
        .context("多重起動防止のインスタンス作成に失敗しました")?;
    if !instance.is_single() {
        let msg = "ClipCoderは既に実行されています。";
        notification::error::show_error_notification("多重起動", msg);
        return Ok(());
    }

    // コーデックの指定がない場合は常時監視モード
    if args.codec.is_none() {
        return tray_app::run_loop();
    }

    // コーデックの指定がある場合は一度だけ実行
    let mut clipboard = Clipboard::new().context("クリップボードの初期化に失敗しました")?;
    coder::process_clipboard(&mut clipboard, args.codec.unwrap());
    Ok(())
}
