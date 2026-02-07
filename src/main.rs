#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod notification;
mod refiner;
mod tray;

use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::Parser;
use single_instance::SingleInstance;

#[cfg(windows)]
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

/// コマンドライン引数
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

/// エントリポイント
///
/// # Returns
/// * `Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す。
fn main() -> Result<()> {
    setup_console();

    let args = Args::parse();

    let _instance = ensure_single_instance()?;

    if let Some(mode) = args.mode {
        run_once(mode)?;
    } else {
        tray::run_loop()?;
    }

    Ok(())
}

/// Windowsの場合、親プロセスのコンソールをアタッチする
///
/// これにより、`cargo run`などで起動した場合にコンソール出力が表示されるようになる。
fn setup_console() {
    #[cfg(windows)]
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// 多重起動を防止し、インスタンスを保持する
///
/// # Returns
/// * `Result<SingleInstance>` - シングルインスタンスであることが確認できた場合、そのインスタンスを返す。
///   既に他のインスタンスが実行中の場合は、通知を表示してプロセスを終了する。
fn ensure_single_instance() -> Result<SingleInstance> {
    let instance = SingleInstance::new("com.y_hirata.clip-refiner")
        .context("多重起動防止のインスタンス作成に失敗しました")?;

    if !instance.is_single() {
        let msg = "ClipRefinerは既に実行されています。";
        notification::show_simple_notification("多重起動", msg);
        // 多重起動時は即座に終了するため、ErrではなくOkの扱いにしつつメッセージを表示
        std::process::exit(0);
    }

    Ok(instance)
}

/// クリップボードの内容を一度だけ加工して終了する
///
/// # Arguments
/// * `mode` - 適用する `refiner::RefineMode`。
///
/// # Returns
/// * `Result<()>` - 処理が正常に完了した場合は `Ok(())` を返す。
fn run_once(mode: refiner::RefineMode) -> Result<()> {
    let mut clipboard = Clipboard::new().context("クリップボードの初期化に失敗しました")?;
    refiner::process_clipboard(&mut clipboard, mode);
    Ok(())
}
