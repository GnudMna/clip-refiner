#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::permissions::restrict_private_dir_permissions;
use crate::consts;
use crate::error::ConfigError;

// ======================================================================
// テスト用設定ディレクトリ
// ======================================================================
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
thread_local! {
    static TEST_CONFIG_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// 既定アプリ起動を抑止してパスを記録するかどうか
    static OPEN_PATH_CAPTURE_ENABLED: Cell<bool> = const { Cell::new(false) };
    /// キャプチャ中に開こうとしたパス
    static CAPTURED_OPEN_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

// ======================================================================
// 設定ディレクトリ
// ======================================================================
/// 設定ディレクトリのパスを取得する
///
/// 優先順位:
/// 1. 環境変数 `CLIP_REFINER_CONFIG_DIR` (非空のとき)
/// 2. テスト用上書き (`set_test_config_dir`, debug/test/`test-helpers` のみ)
/// 3. ポータブルモード (実行ファイル隣に `portable` があるとき `<exe_dir>/data`)
/// 4. `%APPDATA%` 配下の `ClipRefiner`
///
/// # Returns
/// * `Result<PathBuf>` - 設定ディレクトリのパス
///
/// # Errors
/// システムディレクトリの取得に失敗した場合
pub fn get_config_dir() -> Result<PathBuf, ConfigError> {
    if let Some(dir) = env_config_dir_override() {
        return Ok(dir);
    }

    #[cfg(any(test, debug_assertions, feature = "test-helpers"))]
    if let Some(dir) = test_config_dir_override() {
        return Ok(dir);
    }

    if let Some(dir) = portable_config_dir() {
        return Ok(dir);
    }

    Ok(config_base_dir()?.join(consts::APP_NAME))
}

/// 現在の解決結果がポータブルモード由来かどうかを返す
///
/// 環境変数またはテスト用上書きが有効なときは `false`
///
/// # Returns
/// * `bool` - ポータブルの `data` ディレクトリを使うとき `true`
pub fn is_portable_mode() -> bool {
    if env_config_dir_override().is_some() {
        return false;
    }

    #[cfg(any(test, debug_assertions, feature = "test-helpers"))]
    if test_config_dir_override().is_some() {
        return false;
    }

    portable_config_dir().is_some()
}

/// 環境変数 `CLIP_REFINER_CONFIG_DIR` による上書き先を返す
fn env_config_dir_override() -> Option<PathBuf> {
    let value = std::env::var_os(consts::ENV_CONFIG_DIR)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

/// 実行ファイル隣のポータブル設定ディレクトリを返す
fn portable_config_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    portable_config_dir_for_exe_dir(exe_dir)
}

/// 実行ファイルディレクトリからポータブル設定ディレクトリを解決する
///
/// マーカーファイル `portable` があるときのみ `Some(<exe_dir>/data)` を返す
///
/// # Arguments
/// * `exe_dir` - 実行ファイルが置かれているディレクトリ
///
/// # Returns
/// * `Option<PathBuf>` - ポータブル時の設定ディレクトリ。非ポータブルなら `None`
fn portable_config_dir_for_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    let marker = exe_dir.join(consts::PORTABLE_MARKER_FILE);
    if !marker.is_file() {
        return None;
    }
    Some(exe_dir.join(consts::PORTABLE_DATA_DIR_NAME))
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
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub fn set_test_config_dir(dir: PathBuf) {
    TEST_CONFIG_DIR.with(|cell| *cell.borrow_mut() = Some(dir));
}

/// テスト用設定ディレクトリの上書きを解除する
///
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub fn clear_test_config_dir() {
    TEST_CONFIG_DIR.with(|cell| *cell.borrow_mut() = None);
    super::clip_store_key::clear_clip_store_key_cache();
}

#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
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
/// 設定ディレクトリの作成に失敗した場合
pub fn get_config_file_path() -> Result<PathBuf, ConfigError> {
    let config_dir = get_config_dir()?;
    fs::create_dir_all(&config_dir).map_err(ConfigError::CreateDir)?;
    // FAT / exFAT など ACL 非対応ボリュームでは失敗しうる (ポータブル USB 向けに継続)
    if let Err(e) = restrict_private_dir_permissions(&config_dir) {
        tracing::warn!("設定ディレクトリのパーミッション設定に失敗: {:?}", e);
    }
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

/// テスト時に既定アプリ起動を抑止し、開こうとしたパスの記録を開始する
///
/// [`take_captured_open_path_for_test`] で記録を取得して終了する
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub fn begin_capture_open_path_for_test() {
    CAPTURED_OPEN_PATH.with(|cell| *cell.borrow_mut() = None);
    OPEN_PATH_CAPTURE_ENABLED.with(|cell| cell.set(true));
}

/// 記録した起動対象パスを取り出し、キャプチャを終了する
///
/// # Returns
/// * `Option<PathBuf>` - キャプチャ中に開こうとしたパス。未起動なら `None`
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub fn take_captured_open_path_for_test() -> Option<PathBuf> {
    OPEN_PATH_CAPTURE_ENABLED.with(|cell| cell.set(false));
    CAPTURED_OPEN_PATH.with(|cell| cell.borrow_mut().take())
}

/// パスを既定アプリケーションで開く
fn open_path_in_default_application(path: &Path) -> Result<(), ConfigError> {
    #[cfg(any(test, debug_assertions, feature = "test-helpers"))]
    if OPEN_PATH_CAPTURE_ENABLED.with(Cell::get) {
        CAPTURED_OPEN_PATH.with(|cell| *cell.borrow_mut() = Some(path.to_path_buf()));
        return Ok(());
    }

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
    use crate::consts;
    use crate::test_helpers::with_temp_config_dir;

    /// 環境変数 `CLIP_REFINER_CONFIG_DIR` を優先すること
    #[test]
    fn get_config_dir_uses_env_override() {
        let dir =
            std::env::temp_dir().join(format!("clip-refiner-env-config-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        // SAFETY: Windows の環境変数 API はスレッドセーフで、std 経由の読み書きは
        // std 内部のロックで直列化される。環境変数を読む非 Rust コードはテスト中に動かない
        unsafe {
            std::env::set_var(consts::ENV_CONFIG_DIR, &dir);
        }
        let resolved = get_config_dir().expect("get_config_dir");
        // SAFETY: `set_var` と同じ理由で、他スレッドの読み取りと競合しない
        unsafe {
            std::env::remove_var(consts::ENV_CONFIG_DIR);
        }
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(resolved, dir);
    }

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

    /// マーカー `portable` があるとき実行ファイル隣の `data` を返すようにすること
    #[test]
    fn portable_config_dir_uses_data_when_marker_exists() {
        let exe_dir = std::env::temp_dir().join(format!(
            "clip-refiner-portable-marker-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&exe_dir);
        fs::create_dir_all(&exe_dir).expect("create exe dir");
        fs::write(exe_dir.join(consts::PORTABLE_MARKER_FILE), b"").expect("write marker");

        let resolved = portable_config_dir_for_exe_dir(&exe_dir);
        let _ = fs::remove_dir_all(&exe_dir);

        assert_eq!(resolved, Some(exe_dir.join(consts::PORTABLE_DATA_DIR_NAME)));
    }

    /// マーカーが無いときはポータブル設定ディレクトリを返さないようにすること
    #[test]
    fn portable_config_dir_none_without_marker() {
        let exe_dir =
            std::env::temp_dir().join(format!("clip-refiner-portable-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&exe_dir);
        fs::create_dir_all(&exe_dir).expect("create exe dir");

        let resolved = portable_config_dir_for_exe_dir(&exe_dir);
        let _ = fs::remove_dir_all(&exe_dir);

        assert_eq!(resolved, None);
    }

    /// マーカーがディレクトリのときはポータブルとみなさないようにすること
    #[test]
    fn portable_config_dir_none_when_marker_is_directory() {
        let exe_dir = std::env::temp_dir().join(format!(
            "clip-refiner-portable-dir-marker-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&exe_dir);
        fs::create_dir_all(&exe_dir).expect("create exe dir");
        fs::create_dir_all(exe_dir.join(consts::PORTABLE_MARKER_FILE)).expect("create dir marker");

        let resolved = portable_config_dir_for_exe_dir(&exe_dir);
        let _ = fs::remove_dir_all(&exe_dir);

        assert_eq!(resolved, None);
    }

    /// 環境変数上書き中は `is_portable_mode` が偽になるようにすること
    #[test]
    fn is_portable_mode_false_with_env_override() {
        let dir =
            std::env::temp_dir().join(format!("clip-refiner-portable-env-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        // SAFETY: Windows の環境変数 API はスレッドセーフで、std 経由の読み書きは
        // std 内部のロックで直列化される。環境変数を読む非 Rust コードはテスト中に動かない
        unsafe {
            std::env::set_var(consts::ENV_CONFIG_DIR, &dir);
        }
        let portable = is_portable_mode();
        // SAFETY: `set_var` と同じ理由で、他スレッドの読み取りと競合しない
        unsafe {
            std::env::remove_var(consts::ENV_CONFIG_DIR);
        }
        let _ = fs::remove_dir_all(&dir);
        assert!(!portable);
    }

    /// `open_config_file` がキャプチャ時に実プロセスを起動せずパスを記録すること
    #[test]
    fn open_config_file_captures_path_without_spawning() {
        with_temp_config_dir(|| {
            let expected = get_config_file_path().expect("get_config_file_path");
            fs::write(&expected, b"version = 2\n").expect("write config");

            begin_capture_open_path_for_test();
            open_config_file().expect("open_config_file");
            let opened = take_captured_open_path_for_test();

            assert_eq!(opened.as_deref(), Some(expected.as_path()));
        });
    }
}
