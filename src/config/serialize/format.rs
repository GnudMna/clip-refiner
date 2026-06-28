use std::fmt::Write;

use super::docs::SECTION_RULE;
use super::scalar::toml_scalar;

use anyhow::Result;
use serde::Serialize;

/// ルートレベルのセクション見出しを書き出す
pub(crate) fn write_section<W: Write>(out: &mut W, title: &str) -> Result<()> {
    writeln!(out)?;
    writeln!(out, "{SECTION_RULE}")?;
    writeln!(out, "# {title}")?;
    writeln!(out, "{SECTION_RULE}")?;
    writeln!(out)?;
    Ok(())
}

/// テーブルセクションの見出しと `[name]` 行を書き出す
pub(crate) fn write_table_section<W: Write>(
    out: &mut W,
    title: &str,
    table_name: &str,
) -> Result<()> {
    write_section(out, title)?;
    writeln!(out, "[{table_name}]")?;
    writeln!(out)?;
    Ok(())
}

/// キーと値をコメント付きで書き出す
pub(crate) fn write_field<W, T>(
    out: &mut W,
    indent: &str,
    comment: &str,
    key: &str,
    value: &T,
) -> Result<()>
where
    W: Write,
    T: Serialize,
{
    writeln!(out, "{indent}# {comment}")?;
    writeln!(out, "{indent}{key} = {}", toml_scalar(value)?)?;
    writeln!(out)?;
    Ok(())
}

/// セクション見出しの decor 用プレフィックスを返す
pub(crate) fn section_header_prefix(title: &str) -> String {
    format!("\n{SECTION_RULE}\n# {title}\n{SECTION_RULE}\n\n")
}

/// 項目説明コメントの decor 用プレフィックスを返す
pub(crate) fn field_comment_prefix(indent: &str, comment: &str) -> String {
    format!("{indent}# {comment}\n")
}
