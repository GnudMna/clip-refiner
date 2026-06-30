use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::permissions::restrict_private_dir_permissions;

use crate::consts;

use anyhow::{Context, Result};

// ======================================================================
// テスト用設定ディレクトリ
// ======================================================================
#[cfg(any(test, feature = "test-helpers"))]
thread_local! {
    static TEST_CONFIG_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

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
    #[cfg(any(test, feature = "test-helpers"))]
    if let Some(dir) = test_config_dir_override() {
        return Ok(dir);
    }

    let base_dirs =
        directories::BaseDirs::new().context("システムディレクトリの取得に失敗しました")?;

    Ok(base_dirs.config_dir().join(config_dir_name()))
}

/// テスト用に設定ディレクトリを上書きする
#[cfg(any(test, feature = "test-helpers"))]
pub(crate) fn set_test_config_dir(dir: PathBuf) {
    TEST_CONFIG_DIR.with(|cell| *cell.borrow_mut() = Some(dir));
}

/// テスト用設定ディレクトリの上書きを解除する
#[cfg(any(test, feature = "test-helpers"))]
pub(crate) fn clear_test_config_dir() {
    TEST_CONFIG_DIR.with(|cell| *cell.borrow_mut() = None);
    super::clip_store_key::clear_clip_store_key_cache();
}

#[cfg(any(test, feature = "test-helpers"))]
fn test_config_dir_override() -> Option<PathBuf> {
    TEST_CONFIG_DIR.with(|cell| cell.borrow().clone())
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
/// * `Result<PathBuf>` - `config.toml` の完全なパス
pub fn get_config_file_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗しました")?;
    restrict_private_dir_permissions(&config_dir)?;
    Ok(config_dir.join("config.toml"))
}

/// 設定ファイルの最終更新時刻を取得する
///
/// ファイルが存在しない場合は `None` を返す
///
/// # Returns
/// * `Result<Option<SystemTime>>` - 更新時刻。取得失敗時は `Err`
pub fn config_file_modified_time() -> Result<Option<SystemTime>> {
    let path = get_config_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(&path).context("設定ファイルのメタデータ取得に失敗しました")?;
    metadata
        .modified()
        .context("設定ファイルの更新時刻取得に失敗しました")
        .map(Some)
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
