use std::fmt::Write;

use super::super::docs::TABLE_INDENT;
use super::super::docs::{
    DOC_REGEX_CASE_INSENSITIVE, DOC_REGEX_MULTILINE, DOC_REGEX_PATTERN, DOC_REGEX_REPLACEMENT,
    SECTION_REGEX,
};
use super::super::document::{ensure_table, set_table_value};
use super::super::format::{write_field, write_table_section};

use crate::config::types::AppConfig;

use anyhow::{Context, Result};
use toml_edit::DocumentMut;

// ======================================================================
// ドキュメント更新
// ======================================================================
/// `[regex]` の設定値を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    ensure_table(doc, "regex", SECTION_REGEX);
    let regex = doc["regex"]
        .as_table_mut()
        .context("regex テーブルが存在しない")?;
    write_regex_table(regex, config)
}

fn write_regex_table(regex: &mut toml_edit::Table, config: &AppConfig) -> Result<()> {
    set_table_value(
        regex,
        "pattern",
        DOC_REGEX_PATTERN,
        TABLE_INDENT,
        &config.regex.pattern,
    )?;
    set_table_value(
        regex,
        "replacement",
        DOC_REGEX_REPLACEMENT,
        TABLE_INDENT,
        &config.regex.replacement,
    )?;
    set_table_value(
        regex,
        "case_insensitive",
        DOC_REGEX_CASE_INSENSITIVE,
        TABLE_INDENT,
        &config.regex.case_insensitive,
    )?;
    set_table_value(
        regex,
        "multiline",
        DOC_REGEX_MULTILINE,
        TABLE_INDENT,
        &config.regex.multiline,
    )?;
    Ok(())
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
        DOC_REGEX_PATTERN,
        "pattern",
        &config.regex.pattern,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_REGEX_REPLACEMENT,
        "replacement",
        &config.regex.replacement,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_REGEX_CASE_INSENSITIVE,
        "case_insensitive",
        &config.regex.case_insensitive,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_REGEX_MULTILINE,
        "multiline",
        &config.regex.multiline,
    )?;
    Ok(())
}
