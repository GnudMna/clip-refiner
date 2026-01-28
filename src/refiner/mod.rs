pub mod json;
pub mod markdown;
pub mod number;
pub mod sort;
pub mod trim;
pub mod url;
pub mod yaml;

use arboard::Clipboard;
use clap::ValueEnum;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefineMode {
    #[value(help = "URLエンコード")]
    UrlEncode,
    #[value(help = "URLデコード")]
    UrlDecode,
    #[value(help = "UTMパラメータを削除")]
    RemoveUtm,
    #[value(help = "改行や空白を整形")]
    Trim,
    #[value(help = "行単位で改行や空白を整形")]
    TrimLines,
    #[value(help = "MarkdownをHTML形式へ変換")]
    MarkdownToHtml,
    #[value(help = "JSON形式を整形(キー順序不同)")]
    JsonFormat,
    #[value(help = "JSON形式を整形(キー順序保持)")]
    JsonFormatPreserveOrder,
    #[value(help = "JSON形式をYAML形式へ変換(キー順序不同)")]
    JsonToYaml,
    #[value(help = "JSON形式をYAML形式へ変換(キー順序保持)")]
    JsonToYamlPreserveOrder,
    #[value(help = "YAML形式をJSON形式へ変換(キー順序不同)")]
    YamlToJson,
    #[value(help = "YAML形式をJSON形式へ変換(キー順序保持)")]
    YamlToJsonPreserveOrder,
    #[value(help = "カンマ無し数値をカンマ区切りの数値に")]
    AddComma,
    #[value(help = "カンマ区切りの数値をカンマ無し数値に")]
    RemoveComma,
    #[value(help = "行単位で並び替え")]
    SortLines,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RefineCategory {
    Normal,
    JsonFormat,
    JsonToYaml,
    YamlToJson,
}

impl RefineMode {
    pub fn label(&self) -> &'static str {
        match self {
            RefineMode::UrlEncode => "URLエンコード",
            RefineMode::UrlDecode => "URLデコード",
            RefineMode::RemoveUtm => "UTM除去",
            RefineMode::Trim => "トリム",
            RefineMode::TrimLines => "トリム(行単位)",
            RefineMode::MarkdownToHtml => "Markdown→HTML",
            RefineMode::JsonFormat => "キー順序不同",
            RefineMode::JsonFormatPreserveOrder => "キー順序保持",
            RefineMode::JsonToYaml => "キー順序不同",
            RefineMode::JsonToYamlPreserveOrder => "キー順序保持",
            RefineMode::YamlToJson => "キー順序不同",
            RefineMode::YamlToJsonPreserveOrder => "キー順序保持",
            RefineMode::AddComma => "カンマ追加",
            RefineMode::RemoveComma => "カンマ除去",
            RefineMode::SortLines => "行並び替え",
        }
    }

    pub fn category(&self) -> RefineCategory {
        match self {
            RefineMode::JsonFormat | RefineMode::JsonFormatPreserveOrder => {
                RefineCategory::JsonFormat
            }
            RefineMode::JsonToYaml | RefineMode::JsonToYamlPreserveOrder => {
                RefineCategory::JsonToYaml
            }
            RefineMode::YamlToJson | RefineMode::YamlToJsonPreserveOrder => {
                RefineCategory::YamlToJson
            }
            _ => RefineCategory::Normal,
        }
    }

    pub fn variants() -> &'static [RefineMode] {
        &[
            RefineMode::UrlEncode,
            RefineMode::UrlDecode,
            RefineMode::RemoveUtm,
            RefineMode::Trim,
            RefineMode::TrimLines,
            RefineMode::MarkdownToHtml,
            RefineMode::JsonFormat,
            RefineMode::JsonFormatPreserveOrder,
            RefineMode::JsonToYaml,
            RefineMode::JsonToYamlPreserveOrder,
            RefineMode::YamlToJson,
            RefineMode::YamlToJsonPreserveOrder,
            RefineMode::AddComma,
            RefineMode::RemoveComma,
            RefineMode::SortLines,
        ]
    }
}

/// JSON, YAMLキー順序保持用
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrderedValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<OrderedValue>),
    Object(IndexMap<String, OrderedValue>),
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
        RefineMode::TrimLines => trim::trim_lines(&text),
        RefineMode::JsonFormat => json::format_json(&text),
        RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(&text),
        RefineMode::JsonToYaml => json::json_to_yaml(&text),
        RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(&text),
        RefineMode::YamlToJson => yaml::yaml_to_json(&text),
        RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(&text),
        RefineMode::AddComma => number::add_commas(&text),
        RefineMode::RemoveComma => number::remove_commas(&text),
        RefineMode::SortLines => sort::sort_lines(&text),
        RefineMode::MarkdownToHtml => markdown::markdown_to_html(&text),
    };

    if processed != text {
        let _ = clipboard.set_text(processed.clone());
        Some(processed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arboard::Clipboard;

    #[test]
    fn test_refine_mode_metadata() {
        assert_eq!(RefineMode::UrlEncode.label(), "URLエンコード");
        assert_eq!(RefineMode::UrlEncode.category(), RefineCategory::Normal);

        assert_eq!(RefineMode::JsonFormat.label(), "キー順序不同");
        assert_eq!(
            RefineMode::JsonFormat.category(),
            RefineCategory::JsonFormat
        );
    }

    #[test]
    fn test_refine_mode_variants() {
        let variants = RefineMode::variants();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLines));
        assert_eq!(variants.len(), 15);
    }

    #[test]
    fn test_process_clipboard_integration() {
        // 並列実行による干渉を避けるため、1つのテストケースにまとめる
        if let Ok(mut cb) = Clipboard::new() {
            // Case 1: 変化あり
            let unique_str_1 = "  clip_refiner_test_1  ";
            let _ = cb.set_text(unique_str_1.to_string());
            // システムのクリップボードへの反映を待つ必要がある場合があるが、まずはそのまま
            if let Ok(current) = cb.get_text() {
                if current == unique_str_1 {
                    let result = process_clipboard(&mut cb, RefineMode::Trim);
                    assert_eq!(result, Some("clip_refiner_test_1".to_string()));
                } else {
                    eprintln!(
                        "Clipboard content mismatch for unique_str_1. Expected: '{}', Got: '{}'",
                        unique_str_1, current
                    );
                }
            } else {
                eprintln!("Failed to get clipboard text for unique_str_1.");
            }

            // Case 2: 変化なし
            let unique_str_2 = "clip_refiner_test_2";
            let _ = cb.set_text(unique_str_2.to_string());
            if let Ok(current) = cb.get_text() {
                if current == unique_str_2 {
                    let result = process_clipboard(&mut cb, RefineMode::Trim);
                    assert!(result.is_none());
                } else {
                    eprintln!(
                        "Clipboard content mismatch for unique_str_2. Expected: '{}', Got: '{}'",
                        unique_str_2, current
                    );
                }
            } else {
                eprintln!("Failed to get clipboard text for unique_str_2.");
            }
        } else {
            eprintln!("Failed to initialize clipboard. Skipping clipboard integration tests.");
        }
    }
}
