use super::format::{field_comment_prefix, section_header_prefix};
use super::scalar::serde_to_toml_value;

use anyhow::Result;
use serde::Serialize;
use toml_edit::{DocumentMut, Item, Table};

/// テーブルがなければセクション見出し付きで挿入する
pub(crate) fn ensure_table(doc: &mut DocumentMut, name: &str, section_title: &str) {
    if doc.get(name).and_then(Item::as_table).is_some() {
        return;
    }
    let mut table = Table::new();
    table
        .decor_mut()
        .set_prefix(section_header_prefix(section_title));
    doc.insert(name, Item::Table(table));
}

/// テーブル内の値を更新する (既存キーはコメント付きのまま値だけ差し替え)
pub(crate) fn set_table_value<T: Serialize>(
    table: &mut Table,
    key: &str,
    comment: &str,
    indent: &str,
    value: &T,
) -> Result<()> {
    let new_value = serde_to_toml_value(value)?;
    if let Some(item) = table.get_mut(key)
        && let Some(existing) = item.as_value_mut()
    {
        *existing = new_value;
        return Ok(());
    }
    table.insert(key, Item::Value(new_value));
    if let Some(mut key_mut) = table.key_mut(key) {
        key_mut
            .leaf_decor_mut()
            .set_prefix(field_comment_prefix(indent, comment));
    }
    Ok(())
}
