pub mod json;
pub mod number;
pub mod sort;
pub mod trim;
pub mod url;

use arboard::Clipboard;
use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum RefineMode {
    UrlEncode,
    UrlDecode,
    RemoveUtm,
    Trim,
    JsonFormat,
    AddComma,
    RemoveComma,
    SortLines,
}

/// クリップボードの内容を変換
pub fn process_clipboard(clipboard: &mut Clipboard, mode: RefineMode) -> Option<String> {
    let text = clipboard.get_text().ok()?;
    if text.is_empty() {
        return None;
    }

    let processed = match mode {
        RefineMode::UrlEncode => url::url_encode(&text),
        RefineMode::UrlDecode => url::url_decode(&text).unwrap_or_else(|_| text.clone()),
        RefineMode::RemoveUtm => url::remove_utm_params(&text),
        RefineMode::Trim => trim::trim_text(&text),
        RefineMode::JsonFormat => json::format_json(&text),
        RefineMode::AddComma => number::add_commas(&text),
        RefineMode::RemoveComma => number::remove_commas(&text),
        RefineMode::SortLines => sort::sort_lines(&text),
    };

    if processed != text {
        let _ = clipboard.set_text(processed.clone());
        Some(processed)
    } else {
        None
    }
}
