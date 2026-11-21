use crate::error::ConfigError;
use serde::Serialize;
use toml_edit::{DocumentMut, Value};

type Result<T> = std::result::Result<T, ConfigError>;

/// Serde 値を `toml_edit::Value` へ変換する
pub(crate) fn serde_to_toml_value<T: Serialize>(value: &T) -> Result<Value> {
    let line = format!("v={}", toml_scalar(value)?);
    let doc: DocumentMut = line
        .parse()
        .map_err(|e| ConfigError::serialize(format!("TOML スカラー行のパースに失敗: {e}")))?;
    doc["v"]
        .as_value()
        .cloned()
        .ok_or_else(|| ConfigError::serialize("スカラー値が存在しない"))
}

/// TOML のスカラー値をエスケープ済み文字列として返す
pub(crate) fn toml_scalar<T: Serialize>(value: &T) -> Result<String> {
    #[derive(Serialize)]
    struct Row<'a, T: Serialize> {
        v: &'a T,
    }

    let line = toml::to_string(&Row { v: value })
        .map_err(|e| ConfigError::serialize(format!("TOML スカラーのエンコードに失敗: {e}")))?;
    line.split_once('=')
        .map(|(_, scalar)| scalar.trim().to_string())
        .ok_or_else(|| ConfigError::serialize("TOML スカラー行の解析に失敗"))
}
