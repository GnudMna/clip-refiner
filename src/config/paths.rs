use std::fs;
use std::path::{Path, PathBuf};

use crate::consts;

use anyhow::{Context, Result};

// ======================================================================
// 設定ディレクトリ
// ======================================================================
/// 設定ディレクトリのパスを取得する
///
/// OSに応じたアプリケーション設定ディレクトリのパスを計算する
/// Windows は `ClipRefiner`、Linux/macOS は `clip-refiner` を使用する
///
/// # Returns
/// * `Result<PathBuf>` - OSに応じた設定ディレクトリのパス
pub fn get_config_dir() -> Result<PathBuf> {
    let base_dirs =
        directories::BaseDirs::new().context("システムディレクトリの取得に失敗しました")?;

    Ok(base_dirs.config_dir().join(config_dir_name()))
}

/// OS ごとの設定ディレクトリ名を返す
#[cfg(windows)]
fn config_dir_name() -> &'static str {
    consts::APP_NAME
}

/// OS ごとの設定ディレクトリ名を返す
#[cfg(not(windows))]
fn config_dir_name() -> &'static str {
    consts::APP_NAME_KEBAB
}

/// 設定ファイルのパスを取得する
///
/// 設定ディレクトリが存在しない場合は作成する
///
/// # Returns
/// * `Result<PathBuf>` - `config.json` の完全なパス
pub fn get_config_file_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗しました")?;
    #[cfg(unix)]
    restrict_private_dir_permissions(&config_dir)?;
    #[cfg(not(unix))]
    restrict_private_dir_permissions(&config_dir);
    Ok(config_dir.join("config.json"))
}

/// 設定ファイルを既定のアプリケーションで開く
///
/// 呼び出し前に `AppConfig::save` などでファイルを書き出しておくこと
///
/// # Returns
/// * `Result<()>` - 起動に成功した場合は `Ok(())`、失敗した場合は `Err` を返す
pub fn open_config_file() -> Result<()> {
    let path = get_config_file_path()?;
    open_path_in_default_application(&path)
}

/// パスを OS の既定アプリケーションで開く
fn open_path_in_default_application(path: &Path) -> Result<()> {
    let path_str = path
        .to_str()
        .context("設定ファイルのパスを文字列に変換できませんでした")?;

    platform_open_path(path_str)
}

/// パスを OS の既定アプリケーションで開く
#[cfg(windows)]
fn platform_open_path(path_str: &str) -> Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", path_str])
        .spawn()
        .context("設定ファイルの起動コマンドの実行に失敗しました")?;
    Ok(())
}

/// パスを OS の既定アプリケーションで開く
#[cfg(target_os = "macos")]
fn platform_open_path(path_str: &str) -> Result<()> {
    std::process::Command::new("open")
        .arg(path_str)
        .spawn()
        .context("設定ファイルの起動コマンドの実行に失敗しました")?;
    Ok(())
}

/// パスを OS の既定アプリケーションで開く
#[cfg(all(unix, not(target_os = "macos")))]
fn platform_open_path(path_str: &str) -> Result<()> {
    std::process::Command::new("xdg-open")
        .arg(path_str)
        .spawn()
        .context("設定ファイルの起動コマンドの実行に失敗しました")?;
    Ok(())
}

/// パスを OS の既定アプリケーションで開く
#[cfg(not(any(windows, target_os = "macos", unix)))]
fn platform_open_path(_path_str: &str) -> Result<()> {
    anyhow::bail!("このプラットフォームではファイルを開けません")
}

// ======================================================================
// ファイルパーミッション
// ======================================================================
/// 設定ディレクトリを所有者のみアクセス可能にする (Unix)
#[cfg(unix)]
fn restrict_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)
        .context("設定ディレクトリのメタデータ取得に失敗しました")?
        .permissions();
    perms.set_mode(0o700);
    fs::set_permissions(path, perms)
        .context("設定ディレクトリのパーミッション設定に失敗しました")?;
    Ok(())
}

/// 設定ディレクトリを所有者のみアクセス可能にする (非 Unix)
#[cfg(not(unix))]
fn restrict_private_dir_permissions(_path: &Path) {}

/// 設定ファイルを所有者のみ読み書き可能にする (Unix)
#[cfg(unix)]
pub(crate) fn restrict_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)
        .context("設定ファイルのメタデータ取得に失敗しました")?
        .permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms).context("設定ファイルのパーミッション設定に失敗しました")?;
    Ok(())
}

/// 設定ファイルを所有者のみ読み書き可能にする (非 Unix)
#[cfg(not(unix))]
pub(crate) fn restrict_private_file_permissions(_path: &Path) {}
