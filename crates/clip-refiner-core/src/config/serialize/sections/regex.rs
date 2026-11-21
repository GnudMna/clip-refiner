use std::fmt::Write;

use toml_edit::{Array, DocumentMut, InlineTable, Item, Value};

use super::super::docs::TABLE_INDENT;
use super::super::docs::{DOC_REGEX_ACTIVE_PRESET_ID, SECTION_REGEX, doc_regex_presets};
use super::super::document::{ensure_table, set_table_value};
use super::super::format::{field_comment_prefix, write_field, write_table_section};
use crate::config::types::{AppConfig, RegexPreset, RegexStep};
use crate::error::ConfigError;

type Result<T> = std::result::Result<T, ConfigError>;

// ======================================================================
// ドキュメント更新
// ======================================================================
/// `[regex]` の設定値を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    ensure_table(doc, "regex", SECTION_REGEX);
    let regex = doc["regex"]
        .as_table_mut()
        .ok_or_else(|| ConfigError::serialize("regex テーブルが存在しない"))?;

    // 旧フラットキーと別名キーを除去する
    for key in [
        "activePresetId",
        "pattern",
        "replacement",
        "case_insensitive",
        "caseInsensitive",
        "multiline",
    ] {
        regex.remove(key);
    }

    write_regex_table(regex, config)
}

fn write_regex_table(regex: &mut toml_edit::Table, config: &AppConfig) -> Result<()> {
    set_table_value(
        regex,
        "active_preset_id",
        DOC_REGEX_ACTIVE_PRESET_ID,
        TABLE_INDENT,
        &config.regex.active_preset_id,
    )?;
    regex.insert(
        "presets",
        Item::Value(presets_to_toml_value(&config.regex.presets)),
    );
    if let Some(mut key_mut) = regex.key_mut("presets") {
        key_mut
            .leaf_decor_mut()
            .set_prefix(field_comment_prefix(TABLE_INDENT, &doc_regex_presets()));
    }
    Ok(())
}

/// プリセット配列を `snake_case` のインラインテーブル配列へ変換する
fn presets_to_toml_value(presets: &[RegexPreset]) -> Value {
    let mut arr = Array::new();
    for preset in presets {
        let mut table = InlineTable::new();
        table.insert("id", preset.id.as_str().into());
        table.insert("name", preset.name.as_str().into());
        table.insert("steps", steps_to_toml_value(&preset.steps));
        arr.push(Value::InlineTable(table));
    }
    Value::Array(arr)
}

fn steps_to_toml_value(steps: &[RegexStep]) -> Value {
    let mut arr = Array::new();
    for step in steps {
        let mut table = InlineTable::new();
        table.insert("pattern", step.pattern.as_str().into());
        table.insert("replacement", step.replacement.as_str().into());
        table.insert("case_insensitive", step.case_insensitive.into());
        table.insert("multiline", step.multiline.into());
        arr.push(Value::InlineTable(table));
    }
    Value::Array(arr)
}

// ======================================================================
// テンプレート出力
// ======================================================================
/// `[regex]` をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    write_table_section(out, SECTION_REGEX, "regex")?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_REGEX_ACTIVE_PRESET_ID,
        "active_preset_id",
        &config.regex.active_preset_id,
    )?;
    writeln!(out, "{TABLE_INDENT}# {}", doc_regex_presets())?;
    writeln!(
        out,
        "{TABLE_INDENT}presets = {}",
        presets_to_toml_string(&config.regex.presets)
    )?;
    writeln!(out)?;
    Ok(())
}

fn presets_to_toml_string(presets: &[RegexPreset]) -> String {
    presets_to_toml_value(presets).to_string()
}
