#[cfg(any(test, debug_assertions))]
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::permissions::restrict_private_dir_permissions;

use crate::consts;
use crate::error::ConfigError;

// ======================================================================
// テスト用設定ディレクトリ
// ======================================================================
#[cfg(any(test, debug_assertions))]
thread_local! {
    static TEST_CONFIG_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

// ======================================================================
// 設定ディレクトリ
// ======================================================================
/// 設定ディレクトリのパスを取得する
///
/// `%APPDATA%` 配下の `ClipRefiner` ディレクトリを設定ルートとする
///
/// # Returns
/// * `Result<PathBuf>` - 設定ディレクトリのパス
///
/// # Errors
/// システムディレクトリの取得に失敗した場合
pub fn get_config_dir() -> Result<PathBuf, ConfigError> {
    #[cfg(any(test, debug_assertions))]
    if let Some(dir) = test_config_dir_override() {
        return Ok(dir);
    }

    Ok(config_base_dir()?.join(consts::APP_NAME))
}

/// 設定ディレクトリ基底パス (`%APPDATA%`) を返す
///
/// # Errors
/// 環境変数から設定ディレクトリを解決できない場合
fn config_base_dir() -> Result<PathBuf, ConfigError> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or(ConfigError::SystemDir)
}

/// テスト用に設定ディレクトリを上書きする
///
/// # Arguments
/// * `dir` - 上書き先の設定ディレクトリパス
#[cfg(any(test, debug_assertions))]
pub fn set_test_config_dir(dir: PathBuf) {
    TEST_CONFIG_DIR.with(|cell| *cell.borrow_mut() = Some(dir));
}

/// テスト用設定ディレクトリの上書きを解除する
///
#[cfg(any(test, debug_assertions))]
pub fn clear_test_config_dir() {
    TEST_CONFIG_DIR.with(|cell| *cell.borrow_mut() = None);
    super::clip_store_key::clear_clip_store_key_cache();
}

#[cfg(any(test, debug_assertions))]
fn test_config_dir_override() -> Option<PathBuf> {
    TEST_CONFIG_DIR.with(|cell| cell.borrow().clone())
}

/// 設定ファイルのパスを取得する
///
/// 設定ディレクトリが存在しない場合は作成する
///
/// # Returns
/// * `Result<PathBuf>` - `config.toml` の完全なパス
///
/// # Errors
/// 設定ディレクトリの作成またはパーミッション設定に失敗した場合
pub fn get_config_file_path() -> Result<PathBuf, ConfigError> {
    let config_dir = get_config_dir()?;
    fs::create_dir_all(&config_dir).map_err(ConfigError::CreateDir)?;
    restrict_private_dir_permissions(&config_dir)
        .map_err(|e| ConfigError::DirPermissions(e.to_string()))?;
    Ok(config_dir.join("config.toml"))
}

/// 設定ファイルの最終更新時刻を取得する
///
/// ファイルが存在しない場合は `None` を返す
///
/// # Returns
/// * `Result<Option<SystemTime>>` - 更新時刻。取得失敗時は `Err`
///
/// # Errors
/// 設定ファイルパス取得、メタデータ取得、更新時刻取得のいずれかに失敗した場合
pub fn config_file_modified_time() -> Result<Option<SystemTime>, ConfigError> {
    let path = get_config_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(&path).map_err(ConfigError::Metadata)?;
    metadata
        .modified()
        .map_err(ConfigError::ModifiedTime)
        .map(Some)
}

/// 設定ファイルを既定のアプリケーションで開く
///
/// 呼び出し前に `AppConfig::save` などでファイルを書き出しておくこと
///
/// # Returns
/// * `Result<()>` - 起動に成功した場合は `Ok(())`、失敗した場合は `Err` を返す
///
/// # Errors
/// 設定ファイルパスの取得または既定アプリケーションでの起動に失敗した場合
pub fn open_config_file() -> Result<(), ConfigError> {
    let path = get_config_file_path()?;
    open_path_in_default_application(&path)
}

/// パスを既定アプリケーションで開く
fn open_path_in_default_application(path: &Path) -> Result<(), ConfigError> {
    let path_str = path.to_str().ok_or(ConfigError::InvalidPath)?;

    std::process::Command::new("cmd")
        .args(["/C", "start", "", path_str])
        .spawn()
        .map_err(ConfigError::Open)?;
    Ok(())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    use crate::config::{clear_test_config_dir, set_test_config_dir};
    use crate::test_helpers::with_temp_config_dir;

    /// テスト用上書き時は上書き先ディレクトリをそのまま返すようにすること
    #[test]
    fn get_config_dir_uses_test_override() {
        let dir =
            std::env::temp_dir().join(format!("clip-refiner-path-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");

        set_test_config_dir(dir.clone());
        let resolved = get_config_dir().expect("get_config_dir");
        clear_test_config_dir();
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(resolved, dir);
    }

    /// 設定ファイルパスは `config.toml` を指すようにすること
    #[test]
    fn get_config_file_path_points_to_config_toml() {
        with_temp_config_dir(|| {
            let path = get_config_file_path().expect("get_config_file_path");
            assert_eq!(
                path.file_name().and_then(|n| n.to_str()),
                Some("config.toml")
            );
            assert!(path.parent().is_some_and(std::path::Path::exists));
        });
    }

    /// 未作成の設定ファイルは更新時刻 `None` を返すようにすること
    #[test]
    fn config_file_modified_time_none_when_missing() {
        with_temp_config_dir(|| {
            let path = get_config_file_path().expect("get_config_file_path");
            if path.exists() {
                fs::remove_file(&path).expect("remove config");
            }
            assert_eq!(config_file_modified_time().expect("mtime"), None);
        });
    }
}
