use std::fmt::Write;

use super::super::docs::{DOC_TEXT_BODY, DOC_TEXT_LABEL, SECTION_TEXTS};
use super::super::format::{write_field, write_section};
use super::super::scalar::serde_to_toml_value;

use crate::config::types::AppConfig;

use anyhow::Result;
use toml_edit::{DocumentMut, Item, Table};

/// `[[texts]]` の配列を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    use toml_edit::ArrayOfTables;

    if config.texts.is_empty() {
        doc.as_table_mut().remove("texts");
        return Ok(());
    }

    let mut array = ArrayOfTables::new();
    for entry in &config.texts {
        let mut table = Table::new();
        table.insert("label", Item::Value(serde_to_toml_value(&entry.label)?));
        table.insert("text", Item::Value(serde_to_toml_value(&entry.text)?));
        array.push(table);
    }

    doc["texts"] = Item::ArrayOfTables(array);
    Ok(())
}

/// `[[texts]]` をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    if config.texts.is_empty() {
        return Ok(());
    }

    write_section(out, SECTION_TEXTS)?;

    for entry in &config.texts {
        writeln!(out, "[[texts]]")?;
        writeln!(out)?;
        write_field(out, "", DOC_TEXT_LABEL, "label", &entry.label)?;
        write_field(out, "", DOC_TEXT_BODY, "text", &entry.text)?;
    }
    Ok(())
}
