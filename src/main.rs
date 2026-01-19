mod coder;

use std::error::Error;

use coder::encoder;
use coder::decoder;

use arboard::Clipboard;
use clap::{Parser, ValueEnum};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Codec {
    Encode,
    Decode,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "クリップボードのテキストをパーセントエンコード/デコードするツール")]
struct Args {
    /// コーデックの指定
    #[arg(short = 'c', long = "codec", value_enum)]
    codec: Codec,
}

fn main() -> Result<(), Box<dyn Error>> {
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
