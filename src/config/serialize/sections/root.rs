use std::fmt::Write;

use super::super::docs::{
    DOC_FAVORITE_MODES, DOC_HISTORY_ENABLED, DOC_HISTORY_LIMIT, DOC_INTERVAL_MS, DOC_IS_PAUSED,
    DOC_MODE, DOC_MONITOR_MODE, DOC_PIPELINE, DOC_VERSION, SECTION_BASIC, SECTION_HISTORY,
    SECTION_MONITOR,
};
use super::super::document::set_table_value;
use super::super::format::{write_field, write_section};

use crate::config::types::AppConfig;

use anyhow::Result;
use toml_edit::{DocumentMut, Table};

// ======================================================================
// ドキュメント更新
// ======================================================================
/// ルートレベルの設定値を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    let root = doc.as_table_mut();
    write_root_table(root, config)
}

fn write_root_table(root: &mut Table, config: &AppConfig) -> Result<()> {
    set_table_value(root, "version", DOC_VERSION, "", &config.version)?;
    set_table_value(root, "mode", DOC_MODE, "", &config.mode)?;
    set_table_value(root, "pipeline", DOC_PIPELINE, "", &config.pipeline)?;
    set_table_value(
        root,
        "favorite_modes",
        DOC_FAVORITE_MODES,
        "",
        &config.favorite_modes,
    )?;
    set_table_value(
        root,
        "interval_ms",
        DOC_INTERVAL_MS,
        "",
        &config.interval_ms,
    )?;
    set_table_value(
        root,
        "monitor_mode",
        DOC_MONITOR_MODE,
        "",
        &config.monitor_mode,
    )?;
    set_table_value(
        root,
        "history_enabled",
        DOC_HISTORY_ENABLED,
        "",
        &config.history_enabled,
    )?;
    set_table_value(
        root,
        "history_limit",
        DOC_HISTORY_LIMIT,
        "",
        &config.history_limit,
    )?;
    set_table_value(root, "is_paused", DOC_IS_PAUSED, "", &config.is_paused)?;
    Ok(())
}

// ======================================================================
// テンプレート出力
// ======================================================================
/// ルートレベルの設定をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    write_section(out, SECTION_BASIC)?;
    write_field(out, "", DOC_VERSION, "version", &config.version)?;
    write_field(out, "", DOC_MODE, "mode", &config.mode)?;
    write_field(out, "", DOC_PIPELINE, "pipeline", &config.pipeline)?;
    write_field(
        out,
        "",
        DOC_FAVORITE_MODES,
        "favorite_modes",
        &config.favorite_modes,
    )?;

    write_section(out, SECTION_MONITOR)?;
    write_field(out, "", DOC_INTERVAL_MS, "interval_ms", &config.interval_ms)?;
    write_field(
        out,
        "",
        DOC_MONITOR_MODE,
        "monitor_mode",
        &config.monitor_mode,
    )?;
    write_field(out, "", DOC_IS_PAUSED, "is_paused", &config.is_paused)?;

    write_section(out, SECTION_HISTORY)?;
    write_field(
        out,
        "",
        DOC_HISTORY_ENABLED,
        "history_enabled",
        &config.history_enabled,
    )?;
    write_field(
        out,
        "",
        DOC_HISTORY_LIMIT,
        "history_limit",
        &config.history_limit,
    )?;
    Ok(())
}
