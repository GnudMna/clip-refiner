use serde::{Deserialize, Serialize};

// ======================================================================
// 正規表現設定
// ======================================================================
/// 正規表現加工モード用のパターンと置換文字列
///
/// `config.toml` の `[regex]` セクションとして保存される
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegexSettings {
    /// 正規表現パターン
    #[serde(default)]
    pub pattern: String,
    /// 置換文字列 (`RegexReplace` で使用。キャプチャグループは `$1` 形式)
    #[serde(default)]
    pub replacement: String,
    /// 大文字小文字を無視する (`(?i)` 相当)
    #[serde(default)]
    pub case_insensitive: bool,
    /// 複数行モード (`(?m)` 相当)
    #[serde(default)]
    pub multiline: bool,
}
