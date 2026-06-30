use std::fmt::Write;

use super::sections;

use crate::config::types::AppConfig;

use anyhow::Result;

/// `AppConfig` を各項目の説明コメント付き TOML 文字列へ変換する
pub(crate) fn to_commented_toml(config: &AppConfig) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "# ClipRefiner 設定ファイル")?;

    sections::root::append_template(&mut out, config)?;
    sections::notification::append_template(&mut out, config)?;
    sections::hotkeys::append_template(&mut out, config)?;
    sections::regex::append_template(&mut out, config)?;
    sections::clips::append_template(&mut out, config)?;

    Ok(out)
}
