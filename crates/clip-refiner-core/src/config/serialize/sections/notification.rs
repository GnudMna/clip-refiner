use std::fmt::Write;

use toml_edit::DocumentMut;

use super::super::docs::TABLE_INDENT;
use super::super::docs::{
    DOC_NS_ENABLED, DOC_NS_NOTIFY_CONTINUOUS_RECORD, DOC_NS_NOTIFY_FAVORITES,
    DOC_NS_NOTIFY_HISTORY_RESTORE, DOC_NS_NOTIFY_OCR, DOC_NS_NOTIFY_PAUSE, DOC_NS_NOTIFY_PROCESS,
    DOC_NS_NOTIFY_REGISTERED_CLIP, DOC_NS_NOTIFY_RESULT, SECTION_NOTIFICATION,
};
use super::super::document::{ensure_table, set_table_value};
use super::super::format::{write_field, write_table_section};
use crate::config::types::AppConfig;
use crate::error::ConfigError;

type Result<T> = std::result::Result<T, ConfigError>;

// ======================================================================
// ドキュメント更新
// ======================================================================
/// `[notification_settings]` の設定値を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    ensure_table(doc, "notification_settings", SECTION_NOTIFICATION);
    let notification = doc["notification_settings"]
        .as_table_mut()
        .ok_or_else(|| ConfigError::serialize("notification_settings テーブルが存在しない"))?;
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
        "notify_process",
        DOC_NS_NOTIFY_PROCESS,
        TABLE_INDENT,
        &config.notification_settings.notify_process,
    )?;
    set_table_value(
        notification,
        "notify_pause",
        DOC_NS_NOTIFY_PAUSE,
        TABLE_INDENT,
        &config.notification_settings.notify_pause,
    )?;
    set_table_value(
        notification,
        "notify_history_restore",
        DOC_NS_NOTIFY_HISTORY_RESTORE,
        TABLE_INDENT,
        &config.notification_settings.notify_history_restore,
    )?;
    set_table_value(
        notification,
        "notify_ocr",
        DOC_NS_NOTIFY_OCR,
        TABLE_INDENT,
        &config.notification_settings.notify_ocr,
    )?;
    set_table_value(
        notification,
        "notify_registered_clip",
        DOC_NS_NOTIFY_REGISTERED_CLIP,
        TABLE_INDENT,
        &config.notification_settings.notify_registered_clip,
    )?;
    set_table_value(
        notification,
        "notify_favorites",
        DOC_NS_NOTIFY_FAVORITES,
        TABLE_INDENT,
        &config.notification_settings.notify_favorites,
    )?;
    set_table_value(
        notification,
        "notify_continuous_record",
        DOC_NS_NOTIFY_CONTINUOUS_RECORD,
        TABLE_INDENT,
        &config.notification_settings.notify_continuous_record,
    )?;
    set_table_value(
        notification,
        "notify_result",
        DOC_NS_NOTIFY_RESULT,
        TABLE_INDENT,
        &config.notification_settings.notify_result,
    )?;
    Ok(())
}

// ======================================================================
// テンプレート出力
// ======================================================================
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
        DOC_NS_NOTIFY_PROCESS,
        "notify_process",
        &config.notification_settings.notify_process,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_PAUSE,
        "notify_pause",
        &config.notification_settings.notify_pause,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_HISTORY_RESTORE,
        "notify_history_restore",
        &config.notification_settings.notify_history_restore,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_OCR,
        "notify_ocr",
        &config.notification_settings.notify_ocr,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_REGISTERED_CLIP,
        "notify_registered_clip",
        &config.notification_settings.notify_registered_clip,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_FAVORITES,
        "notify_favorites",
        &config.notification_settings.notify_favorites,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_CONTINUOUS_RECORD,
        "notify_continuous_record",
        &config.notification_settings.notify_continuous_record,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_RESULT,
        "notify_result",
        &config.notification_settings.notify_result,
    )?;
    Ok(())
}
