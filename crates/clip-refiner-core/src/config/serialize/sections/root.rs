use std::fmt::Write;

use toml_edit::{DocumentMut, Table};

use super::super::docs::{
    DOC_AUTO_REFINE_ENABLED, DOC_CONTINUOUS_RECORD_ENABLED, DOC_HISTORY_ENABLED, DOC_MODE,
    DOC_MONITOR_MODE, DOC_SELECTOR_WEBVIEW_RETENTION, DOC_VERSION, SECTION_BASIC,
    SECTION_CONTINUOUS_RECORD, SECTION_HISTORY, SECTION_MONITOR, SECTION_SELECTOR_WEBVIEW,
    doc_continuous_record_limit, doc_favorite_modes, doc_history_limit, doc_interval_ms,
    doc_pipeline, doc_selector_webview_idle_minutes,
};
use super::super::document::set_table_value;
use super::super::format::{write_field, write_section};
use crate::config::types::AppConfig;
use crate::error::ConfigError;

type Result<T> = std::result::Result<T, ConfigError>;

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
    set_table_value(
        root,
        "auto_refine_enabled",
        DOC_AUTO_REFINE_ENABLED,
        "",
        &config.auto_refine_enabled,
    )?;
    set_table_value(root, "mode", DOC_MODE, "", &config.mode)?;
    set_table_value(root, "pipeline", &doc_pipeline(), "", &config.pipeline)?;
    set_table_value(
        root,
        "favorite_modes",
        &doc_favorite_modes(),
        "",
        &config.favorite_modes,
    )?;
    set_table_value(
        root,
        "interval_ms",
        &doc_interval_ms(),
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
        "selector_webview_retention",
        DOC_SELECTOR_WEBVIEW_RETENTION,
        "",
        &config.selector_webview_retention,
    )?;
    set_table_value(
        root,
        "selector_webview_idle_minutes",
        &doc_selector_webview_idle_minutes(),
        "",
        &config.selector_webview_idle_minutes,
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
        "continuous_record_enabled",
        DOC_CONTINUOUS_RECORD_ENABLED,
        "",
        &config.continuous_record_enabled,
    )?;
    set_table_value(
        root,
        "continuous_record_limit",
        &doc_continuous_record_limit(),
        "",
        &config.continuous_record_limit,
    )?;
    set_table_value(
        root,
        "history_limit",
        &doc_history_limit(),
        "",
        &config.history_limit,
    )?;
    // 旧キーを除去 (v5 で廃止。一時停止は自動変換の無効化へ統合)
    root.remove("is_paused");
    Ok(())
}

// ======================================================================
// テンプレート出力
// ======================================================================
/// ルートレベルの設定をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    write_section(out, SECTION_BASIC)?;
    write_field(out, "", DOC_VERSION, "version", &config.version)?;
    write_field(
        out,
        "",
        DOC_AUTO_REFINE_ENABLED,
        "auto_refine_enabled",
        &config.auto_refine_enabled,
    )?;
    write_field(out, "", DOC_MODE, "mode", &config.mode)?;
    write_field(out, "", &doc_pipeline(), "pipeline", &config.pipeline)?;
    write_field(
        out,
        "",
        &doc_favorite_modes(),
        "favorite_modes",
        &config.favorite_modes,
    )?;

    write_section(out, SECTION_MONITOR)?;
    write_field(
        out,
        "",
        &doc_interval_ms(),
        "interval_ms",
        &config.interval_ms,
    )?;
    write_field(
        out,
        "",
        DOC_MONITOR_MODE,
        "monitor_mode",
        &config.monitor_mode,
    )?;

    write_section(out, SECTION_SELECTOR_WEBVIEW)?;
    write_field(
        out,
        "",
        DOC_SELECTOR_WEBVIEW_RETENTION,
        "selector_webview_retention",
        &config.selector_webview_retention,
    )?;
    write_field(
        out,
        "",
        &doc_selector_webview_idle_minutes(),
        "selector_webview_idle_minutes",
        &config.selector_webview_idle_minutes,
    )?;

    write_section(out, SECTION_CONTINUOUS_RECORD)?;
    write_field(
        out,
        "",
        DOC_CONTINUOUS_RECORD_ENABLED,
        "continuous_record_enabled",
        &config.continuous_record_enabled,
    )?;
    write_field(
        out,
        "",
        &doc_continuous_record_limit(),
        "continuous_record_limit",
        &config.continuous_record_limit,
    )?;

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
        &doc_history_limit(),
        "history_limit",
        &config.history_limit,
    )?;
    Ok(())
}
