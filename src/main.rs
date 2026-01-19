#![cfg_attr(windows, windows_subsystem = "windows")]

mod coder;

use std::error::Error;

use coder::decoder;
use coder::encoder;

use arboard::Clipboard;
use clap::{Parser, ValueEnum};

#[cfg(windows)]
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Codec {
    Encode,
    Decode,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "クリップボードのテキストをパーセントエンコード/デコードするツール"
)]
struct Args {
    /// コーデックの指定
    #[arg(short = 'c', long = "codec", value_enum)]
    codec: Codec,
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let args = Args::parse();

    let mut clipboard = Clipboard::new()?;
    let text = clipboard.get_text()?;

    let result = match args.codec {
        Codec::Encode => encoder::percent_encode_text(&text),
        Codec::Decode => decoder::percent_decode_text(&text)?,
    };

    clipboard.set_text(result)?;
    Ok(())
}
