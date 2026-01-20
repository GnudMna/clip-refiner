pub mod decoder;
pub mod encoder;

use arboard::Clipboard;
use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum CodecMode {
    Encode,
    Decode,
}

/// クリップボードの内容を変換
pub fn process_clipboard(clipboard: &mut Clipboard, mode: CodecMode) -> Option<String> {
    let text = clipboard.get_text().ok()?;
    if text.is_empty() {
        return None;
    }

    let processed = match mode {
        CodecMode::Encode => encoder::percent_encode_text(&text),
        CodecMode::Decode => decoder::percent_decode_text(&text).unwrap_or_else(|_| text.clone()),
    };

    if processed != text {
        let _ = clipboard.set_text(processed.clone());
        Some(processed)
    } else {
        None
    }
}
