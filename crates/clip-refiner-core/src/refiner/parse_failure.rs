// ======================================================================
// パース失敗の理由
// ======================================================================
/// JSON / YAML の変換が、入力を読み取れず・変換できず打ち切られた理由
///
/// 変更がなかった場合と区別して通知するために使う
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseFailure {
    /// JSON として不正な入力
    InvalidJson,
    /// YAML として不正な入力
    InvalidYaml,
    /// 同じオブジェクトにキーが重複している
    DuplicateKey,
    /// YAML の `.inf` / `.nan` など、JSON で表現できない数値を含む
    NonFiniteNumber,
    /// 範囲外の数値など、変換先の形式で表現できない値を含む
    UnrepresentableValue,
}

impl ParseFailure {
    /// ユーザー向けの理由を返す
    ///
    /// # Arguments
    /// * `self` - この `ParseFailure`
    ///
    /// # Returns
    /// * `&'static str` - 通知に出す表示用文字列
    #[must_use]
    pub fn user_message(self) -> &'static str {
        match self {
            Self::InvalidJson => "JSON として解釈できないため加工しませんでした",
            Self::InvalidYaml => "YAML として解釈できないため加工しませんでした",
            Self::DuplicateKey => "キーが重複しているため加工しませんでした",
            Self::NonFiniteNumber => {
                "YAML の .inf / .nan は JSON で表現できないため加工しませんでした"
            }
            Self::UnrepresentableValue => {
                "変換先の形式で表現できない値 (範囲外の数値など) を含むため加工しませんでした"
            }
        }
    }
}
