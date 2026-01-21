pub mod trim;
pub mod url;

use arboard::Clipboard;
use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum RefineMode {
    Encode,
    Decode,
    Trim,
}

/// クリップボードの内容を変換
pub fn process_clipboard(clipboard: &mut Clipboard, mode: RefineMode) -> Option<String> {
    let text = clipboard.get_text().ok()?;
    if text.is_empty() {
        return None;
    }

    let processed = match mode {
        RefineMode::Encode => url::encode(&text),
        RefineMode::Decode => url::decode(&text).unwrap_or_else(|_| text.clone()),
        RefineMode::Trim => trim::trim_text(&text),
    };

    if processed != text {
        let _ = clipboard.set_text(processed.clone());
        Some(processed)
    } else {
        None
    }
}
