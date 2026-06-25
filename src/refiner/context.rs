use crate::config::{AppConfig, RegexSettings};

// ======================================================================
// 加工コンテキスト
// ======================================================================
/// 設定依存の加工モード向けコンテキスト
///
/// 正規表現パターンなど、テキスト以外に必要なパラメータを保持する
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefineContext {
    /// 正規表現加工用の設定
    pub regex: RegexSettings,
}

impl RefineContext {
    /// 設定ファイルの内容からコンテキストを生成する
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            regex: config.regex.clone(),
        }
    }
}
