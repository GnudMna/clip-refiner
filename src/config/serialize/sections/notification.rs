use std::fmt::Write;

use super::super::docs::TABLE_INDENT;
use super::super::docs::{
    DOC_NS_ENABLED, DOC_NS_NOTIFY_MODE, DOC_NS_NOTIFY_PAUSE, DOC_NS_NOTIFY_RESULT,
    SECTION_NOTIFICATION,
};
use super::super::document::{ensure_table, set_table_value};
use super::super::format::{write_field, write_table_section};

use crate::config::types::AppConfig;

use anyhow::{Context, Result};
use toml_edit::DocumentMut;

/// `[notification_settings]` の設定値を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    ensure_table(doc, "notification_settings", SECTION_NOTIFICATION);
    let notification = doc["notification_settings"]
        .as_table_mut()
        .context("notification_settings テーブルが存在しない")?;
    write_notification_table(notification, config)
}

fn write_notification_table(notification: &mut toml_edit::Table, config: &AppConfig) -> Result<()> {
    set_table_value(
        notification,
        "enabled",
        DOC_NS_ENABLED,
        TABLE_INDENT,
        &config.notification_settings.enabled,
    )?;
    set_table_value(
        notification,
        "notify_mode",
        DOC_NS_NOTIFY_MODE,
        TABLE_INDENT,
        &config.notification_settings.notify_mode,
    )?;
    set_table_value(
        notification,
        "notify_result",
        DOC_NS_NOTIFY_RESULT,
        TABLE_INDENT,
        &config.notification_settings.notify_result,
    )?;
    set_table_value(
        notification,
        "notify_pause",
        DOC_NS_NOTIFY_PAUSE,
        TABLE_INDENT,
        &config.notification_settings.notify_pause,
    )?;
    Ok(())
}

/// `[notification_settings]` をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    write_table_section(out, SECTION_NOTIFICATION, "notification_settings")?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_ENABLED,
        "enabled",
        &config.notification_settings.enabled,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_MODE,
        "notify_mode",
        &config.notification_settings.notify_mode,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_RESULT,
        "notify_result",
        &config.notification_settings.notify_result,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_PAUSE,
        "notify_pause",
        &config.notification_settings.notify_pause,
    )?;
    Ok(())
}
