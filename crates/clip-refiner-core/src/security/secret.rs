use zeroize::Zeroizing;

// ======================================================================
// 機密文字列
// ======================================================================
/// 破棄時にメモリをゼロ化する文字列
pub type SecretString = Zeroizing<String>;

/// 平文から `SecretString` を生成する
///
/// # Arguments
/// * `text` - 格納する平文
///
/// # Returns
/// * `SecretString` - 破棄時にゼロ化される文字列
pub fn secret_from(text: impl Into<String>) -> SecretString {
    Zeroizing::from(text.into())
}
