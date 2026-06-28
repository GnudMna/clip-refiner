use std::fmt::Write;

use super::super::docs::TABLE_INDENT;
use super::super::docs::{
    DOC_HOTKEY_FAVORITE_SLOTS, DOC_HOTKEY_NOTIFICATION, DOC_HOTKEY_OCR, DOC_HOTKEY_PAUSE,
    DOC_HOTKEY_QUIT, DOC_HOTKEY_SELECTOR, DOC_HOTKEY_TEXT_SELECTOR, DOC_HOTKEY_UNDO,
    SECTION_HOTKEYS,
};
use super::super::document::{ensure_table, set_table_value};
use super::super::format::{write_field, write_table_section};

use crate::config::types::AppConfig;

use anyhow::{Context, Result};
use toml_edit::DocumentMut;

// ======================================================================
// ドキュメント更新
// ======================================================================
/// `[hotkeys]` の設定値を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    ensure_table(doc, "hotkeys", SECTION_HOTKEYS);
    let hotkeys = doc["hotkeys"]
        .as_table_mut()
        .context("hotkeys テーブルが存在しない")?;
    write_hotkeys_table(hotkeys, config)
}

fn write_hotkeys_table(hotkeys: &mut toml_edit::Table, config: &AppConfig) -> Result<()> {
    set_table_value(
        hotkeys,
        "quick_selector",
        DOC_HOTKEY_SELECTOR,
        TABLE_INDENT,
        &config.hotkeys.quick_selector,
    )?;
    hotkeys.remove("selector");
    set_table_value(
        hotkeys,
        "notification",
        DOC_HOTKEY_NOTIFICATION,
        TABLE_INDENT,
        &config.hotkeys.notification,
    )?;
    set_table_value(
        hotkeys,
        "pause",
        DOC_HOTKEY_PAUSE,
        TABLE_INDENT,
        &config.hotkeys.pause,
    )?;
    set_table_value(
        hotkeys,
        "undo",
        DOC_HOTKEY_UNDO,
        TABLE_INDENT,
        &config.hotkeys.undo,
    )?;
    set_table_value(
        hotkeys,
        "text_selector",
        DOC_HOTKEY_TEXT_SELECTOR,
        TABLE_INDENT,
        &config.hotkeys.text_selector,
    )?;
    set_table_value(
        hotkeys,
        "ocr",
        DOC_HOTKEY_OCR,
        TABLE_INDENT,
        &config.hotkeys.ocr,
    )?;
    set_table_value(
        hotkeys,
        "quit",
        DOC_HOTKEY_QUIT,
        TABLE_INDENT,
        &config.hotkeys.quit,
    )?;
    if config.hotkeys.favorite_mode_slots.is_empty() {
        hotkeys.remove("favorite_mode_slots");
    } else {
        set_table_value(
            hotkeys,
            "favorite_mode_slots",
            DOC_HOTKEY_FAVORITE_SLOTS,
            TABLE_INDENT,
            &config.hotkeys.favorite_mode_slots,
        )?;
    }
    Ok(())
}

// ======================================================================
// テンプレート出力
// ======================================================================
/// `[hotkeys]` をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    write_table_section(out, SECTION_HOTKEYS, "hotkeys")?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_SELECTOR,
        "quick_selector",
        &config.hotkeys.quick_selector,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_NOTIFICATION,
        "notification",
        &config.hotkeys.notification,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_PAUSE,
        "pause",
        &config.hotkeys.pause,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_UNDO,
        "undo",
        &config.hotkeys.undo,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_TEXT_SELECTOR,
        "text_selector",
        &config.hotkeys.text_selector,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_OCR,
        "ocr",
        &config.hotkeys.ocr,
    )?;
    write_field(
        out,
        TABLE_INDENT,
        DOC_HOTKEY_QUIT,
        "quit",
        &config.hotkeys.quit,
    )?;
    if !config.hotkeys.favorite_mode_slots.is_empty() {
        write_field(
            out,
            TABLE_INDENT,
            DOC_HOTKEY_FAVORITE_SLOTS,
            "favorite_mode_slots",
            &config.hotkeys.favorite_mode_slots,
        )?;
    }
    Ok(())
}
