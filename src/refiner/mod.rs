pub mod json;
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
    #[value(help = "JSON形式を整形")]
    JsonFormat,
    #[value(help = "JSON形式をYAML形式へ変換(キー順序ソート)")]
    JsonToYaml,
    #[value(help = "JSON形式をYAML形式へ変換(キー順序保持)")]
    JsonToYamlPreserveOrder,
    #[value(help = "YAML形式をJSON形式へ変換(キー順序ソート)")]
    YamlToJsonPreserveOrder,
    #[value(help = "YAML形式をJSON形式へ変換(キー順序保持)")]
    YamlToJson,
    #[value(help = "カンマ無し数値をカンマ区切りの数値に")]
    AddComma,
    #[value(help = "カンマ区切りの数値をカンマ無し数値に")]
    RemoveComma,
    #[value(help = "行単位で並び替え")]
    SortLines,
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
        RefineMode::JsonToYaml => json::json_to_yaml(&text),
        RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(&text),
        RefineMode::YamlToJson => yaml::yaml_to_json(&text),
        RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(&text),
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
