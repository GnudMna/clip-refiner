use std::fmt::Write;

use super::super::docs::{DOC_CLIP_BODY, DOC_CLIP_IMAGE, DOC_CLIP_LABEL, SECTION_CLIPS};
use super::super::format::{write_field, write_section};
use super::super::scalar::serde_to_toml_value;

use crate::config::types::AppConfig;

use anyhow::Result;
use toml_edit::{DocumentMut, Item, Table};

/// `[[clips]]` の配列を更新する
pub(crate) fn apply(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    use toml_edit::ArrayOfTables;

    if config.clips.is_empty() {
        doc.as_table_mut().remove("clips");
        return Ok(());
    }

    let mut array = ArrayOfTables::new();
    for entry in &config.clips {
        let mut table = Table::new();
        table.insert("label", Item::Value(serde_to_toml_value(&entry.label)?));
        table.insert("text", Item::Value(serde_to_toml_value(&entry.text)?));
        if let Some(ref image_file) = entry.image_file {
            table.insert("image_file", Item::Value(serde_to_toml_value(image_file)?));
        }
        array.push(table);
    }

    doc["clips"] = Item::ArrayOfTables(array);
    Ok(())
}

/// `[[clips]]` をコメント付きテンプレートへ書き出す
pub(crate) fn append_template<W: Write>(out: &mut W, config: &AppConfig) -> Result<()> {
    if config.clips.is_empty() {
        return Ok(());
    }

    write_section(out, SECTION_CLIPS)?;

    for entry in &config.clips {
        writeln!(out, "[[clips]]")?;
        writeln!(out)?;
        write_field(out, "", DOC_CLIP_LABEL, "label", &entry.label)?;
        write_field(out, "", DOC_CLIP_BODY, "text", &entry.text)?;
        if let Some(ref image_file) = entry.image_file {
            write_field(out, "", DOC_CLIP_IMAGE, "image_file", image_file)?;
        }
    }
    Ok(())
}
