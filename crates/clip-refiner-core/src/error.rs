//! ライブラリ公開 API のエラー型

use thiserror::Error;

pub use crate::config::AddRegisteredClipError;
pub use crate::config::ConfigReloadError;
pub use crate::hotkey_binding::HotkeyParseError;
pub use crate::refiner::{ClipboardIoError, ClipboardProcessError};

// ======================================================================
// 設定 I/O
// ======================================================================
/// 設定ディレクトリ・ファイル操作の失敗理由
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
}

// ======================================================================
// クレート共通
// ======================================================================
/// `clip-refiner-core` 公開 API の代表的な失敗理由
#[derive(Debug, Error)]
pub enum Error {
    /// 設定ファイル I/O
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// 設定ファイルの再読み込み
    #[error(transparent)]
    ConfigReload(#[from] ConfigReloadError),
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
    #[error(transparent)]
    AddRegisteredClip(#[from] AddRegisteredClipError),
}
