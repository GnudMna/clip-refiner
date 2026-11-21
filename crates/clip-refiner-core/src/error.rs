//! ライブラリ公開 API のエラー型

#[cfg(feature = "config")]
pub use crate::config::AddRegisteredClipError;
#[cfg(feature = "config")]
pub use crate::config::ConfigReloadError;
pub use crate::hotkey_binding::HotkeyParseError;
pub use crate::refiner::{ClipboardIoError, ClipboardProcessError};

use thiserror::Error;

// ======================================================================
// 暗号化
// ======================================================================
/// 登録クリップ・履歴向け AEAD の失敗理由
#[cfg(feature = "config")]
#[derive(Debug, Error)]
pub enum CipherError {
    /// OS 乱数による鍵生成に失敗
    #[error("暗号化鍵の生成に失敗しました")]
    KeyGenerate(#[source] getrandom::Error),
    /// OS 乱数によるノンス生成に失敗
    #[error("暗号化ノンスの生成に失敗しました")]
    NonceGenerate(#[source] getrandom::Error),
    /// AEAD 暗号化に失敗
    #[error("暗号化に失敗しました")]
    Encrypt,
    /// AEAD 復号に失敗
    #[error("復号に失敗しました")]
    Decrypt,
    /// 暗号文がノンス長以下
    #[error("暗号文が短すぎます")]
    CiphertextTooShort,
    /// 鍵バイト列の長さが不正
    #[error("不正な鍵長です")]
    InvalidKeyLength,
    /// ノンスバイト列の長さが不正
    #[error("不正なノンス長です")]
    InvalidNonceLength,
}

// ======================================================================
// 設定 I/O
// ======================================================================
/// 設定ディレクトリ・ファイル操作の失敗理由
#[cfg(feature = "config")]
#[derive(Debug, Error)]
pub enum ConfigError {
    /// システム設定ディレクトリの取得に失敗
    #[error("システムディレクトリの取得に失敗しました")]
    SystemDir,
    /// 設定ディレクトリの作成に失敗
    #[error("設定ディレクトリの作成に失敗しました")]
    CreateDir(#[source] std::io::Error),
    /// 設定ディレクトリのパーミッション設定に失敗
    #[error("設定ディレクトリのパーミッション設定に失敗しました: {0}")]
    DirPermissions(String),
    /// 設定ファイルの読み取りに失敗
    #[error("設定ファイルの読み取りに失敗しました")]
    Read(#[source] std::io::Error),
    /// 設定ファイルの書き込みに失敗
    #[error("設定ファイルの書き込みに失敗しました")]
    Write(#[source] std::io::Error),
    /// 設定ファイルのメタデータ取得に失敗
    #[error("設定ファイルのメタデータ取得に失敗しました")]
    Metadata(#[source] std::io::Error),
    /// 設定ファイルの更新時刻取得に失敗
    #[error("設定ファイルの更新時刻取得に失敗しました")]
    ModifiedTime(#[source] std::io::Error),
    /// TOML シリアライズに失敗
    #[error("設定のシリアライズに失敗しました: {0}")]
    Serialize(String),
    /// 登録クリップファイルの保存に失敗
    #[error("登録クリップの保存に失敗しました: {0}")]
    RegisteredClipsSave(String),
    /// 設定ファイルのパスを文字列化できない
    #[error("設定ファイルのパスを文字列に変換できませんでした")]
    InvalidPath,
    /// 既定アプリケーションでの起動に失敗
    #[error("設定ファイルを開けませんでした")]
    Open(#[source] std::io::Error),
    /// 設定ファイルのパーミッション設定に失敗
    #[error("設定ファイルのパーミッション設定に失敗しました: {0}")]
    FilePermissions(String),
    /// DPAPI による保護または復号に失敗
    #[error("DPAPI の処理に失敗しました: {0}")]
    Dpapi(String),
    /// AEAD 暗号化
    #[error(transparent)]
    Cipher(#[from] CipherError),
}

#[cfg(feature = "config")]
impl ConfigError {
    /// シリアライズ失敗を生成する
    pub(crate) fn serialize(msg: impl Into<String>) -> Self {
        Self::Serialize(msg.into())
    }

    /// DPAPI 失敗を生成する
    pub(crate) fn dpapi(msg: impl Into<String>) -> Self {
        Self::Dpapi(msg.into())
    }
}

#[cfg(feature = "config")]
impl From<std::fmt::Error> for ConfigError {
    fn from(err: std::fmt::Error) -> Self {
        Self::Serialize(err.to_string())
    }
}

// ======================================================================
// クレート共通
// ======================================================================
/// `clip-refiner-core` 公開 API の代表的な失敗理由
#[derive(Debug, Error)]
pub enum Error {
    /// 設定ファイル I/O
    #[cfg(feature = "config")]
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// 設定ファイルの再読み込み
    #[cfg(feature = "config")]
    #[error(transparent)]
    ConfigReload(#[from] ConfigReloadError),
    /// AEAD 暗号化
    #[cfg(feature = "config")]
    #[error(transparent)]
    Cipher(#[from] CipherError),
    /// クリップボード I/O
    #[error(transparent)]
    ClipboardIo(#[from] ClipboardIoError),
    /// クリップボード加工
    #[error(transparent)]
    ClipboardProcess(#[from] ClipboardProcessError),
    /// ホットキー文字列の解析
    #[error(transparent)]
    HotkeyParse(#[from] HotkeyParseError),
    /// 登録クリップの追加
    #[cfg(feature = "config")]
    #[error(transparent)]
    AddRegisteredClip(#[from] AddRegisteredClipError),
}
