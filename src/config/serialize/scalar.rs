use anyhow::{Context, Result};
use serde::Serialize;
use toml_edit::{DocumentMut, Value};

/// Serde 値を `toml_edit::Value` へ変換する
pub(crate) fn serde_to_toml_value<T: Serialize>(value: &T) -> Result<Value> {
    let line = format!("v={}", toml_scalar(value)?);
    let doc: DocumentMut = line.parse().context("TOML スカラー行のパースに失敗")?;
    doc["v"]
        .as_value()
        .context("スカラー値が存在しない")
        .cloned()
}

/// TOML のスカラー値をエスケープ済み文字列として返す
pub(crate) fn toml_scalar<T: Serialize>(value: &T) -> Result<String> {
    #[derive(Serialize)]
    struct Row<'a, T: Serialize> {
        v: &'a T,
    }

    let line = toml::to_string(&Row { v: value }).context("TOML スカラーのエンコードに失敗")?;
    line.split_once('=')
        .map(|(_, scalar)| scalar.trim().to_string())
        .context("TOML スカラー行の解析に失敗")
}
