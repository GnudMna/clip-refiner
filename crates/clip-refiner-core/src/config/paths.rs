#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

use super::permissions::restrict_private_dir_permissions;
use crate::consts;
use crate::error::ConfigError;

// ======================================================================
// 定数
// ======================================================================
/// `ShellExecuteW` が成功時に返す値の下限 (これ以下はエラーコード)
const SHELL_EXECUTE_SUCCESS_MIN: isize = 32;

/// パーミッション設定を試みた設定ディレクトリ (成否は問わない)
static RESTRICTED_CONFIG_DIRS: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

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
    resolve_config_dir(env_config_dir_override())
}

/// 環境変数の上書き値を受け取って設定ディレクトリを解決する
///
/// テストが環境変数 (プロセス全体で共有) を書き換えずに優先順位を検証できるよう分離している
///
/// # Arguments
/// * `env_dir` - 環境変数による上書き先。未設定または空なら `None`
///
/// # Returns
/// * `Result<PathBuf>` - 設定ディレクトリのパス
///
/// # Errors
/// システムディレクトリの取得に失敗した場合
fn resolve_config_dir(env_dir: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    if let Some(dir) = env_dir {
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
    portable_mode_with(env_config_dir_override().as_deref())
}

/// 環境変数の上書き値を受け取ってポータブルモードかどうかを判定する
///
/// # Arguments
/// * `env_dir` - 環境変数による上書き先。未設定または空なら `None`
///
/// # Returns
/// * `bool` - ポータブルの `data` ディレクトリを使うとき `true`
fn portable_mode_with(env_dir: Option<&Path>) -> bool {
    if env_dir.is_some() {
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
/// キャッシュ済みの登録クリップ暗号化鍵も破棄する
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
/// 所有者専用のパーミッションは、プロセス内で設定ディレクトリごとに初回 (作成時を含む) のみ設定する
/// 外部変更の確認などで繰り返し呼ばれるため、失敗時の警告ログと継承の再伝播を避ける
///
/// # Returns
/// * `Result<PathBuf>` - `config.toml` の完全なパス
///
/// # Errors
/// 設定ディレクトリの取得または作成に失敗した場合
pub fn get_config_file_path() -> Result<PathBuf, ConfigError> {
    let config_dir = get_config_dir()?;
    let newly_created = !config_dir.is_dir();
    fs::create_dir_all(&config_dir).map_err(ConfigError::CreateDir)?;
    if should_restrict_config_dir(&config_dir, newly_created) {
        // FAT / exFAT など ACL 非対応ボリュームでは失敗しうる (ポータブル USB 向けに継続)
        if let Err(e) = restrict_private_dir_permissions(&config_dir) {
            tracing::warn!("設定ディレクトリのパーミッション設定に失敗: {:?}", e);
        }
    }
    Ok(config_dir.join("config.toml"))
}

/// 設定ディレクトリへパーミッションを設定すべきかを返す
///
/// プロセス内で初めて扱うディレクトリ、またはこの呼び出しで作成したディレクトリのとき `true`
/// 設定の成否にかかわらず記録するため、ACL 非対応ボリュームでも再試行しない
///
/// # Arguments
/// * `dir` - 設定ディレクトリのパス
/// * `newly_created` - 呼び出し直前まで存在せず、今回作成したかどうか
///
/// # Returns
/// * `bool` - パーミッションを設定すべきとき `true`
fn should_restrict_config_dir(dir: &Path, newly_created: bool) -> bool {
    let mut guard = RESTRICTED_CONFIG_DIRS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let first_time = guard
        .get_or_insert_with(HashSet::new)
        .insert(dir.to_path_buf());
    first_time || newly_created
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
    let file = HSTRING::from(path_str);

    // `cmd /C start` は `&` や `%` を含むパスを再解釈するため、`ShellExecuteW` に直接渡す
    // SAFETY: `file` は NUL 終端の UTF-16 で呼び出し中は生存し、そのほかの引数は NULL か定数
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &file,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // 戻り値は `HINSTANCE` 型だが、32 以下ならエラーコード
    let code = result.0 as isize;
    if code > SHELL_EXECUTE_SUCCESS_MIN {
        return Ok(());
    }
    Err(ConfigError::Open(std::io::Error::from_raw_os_error(
        i32::try_from(code).unwrap_or(0),
    )))
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

    /// 環境変数による上書きをテスト用上書きより優先すること
    ///
    /// 環境変数はプロセス全体で共有され並列テストに影響するため、値は引数で渡す
    #[test]
    fn get_config_dir_uses_env_override() {
        let env_dir = PathBuf::from("env-config-dir");
        let test_dir = PathBuf::from("test-config-dir");

        set_test_config_dir(test_dir);
        let resolved = resolve_config_dir(Some(env_dir.clone())).expect("resolve_config_dir");
        clear_test_config_dir();

        assert_eq!(resolved, env_dir);
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

    /// パーミッション設定は同一ディレクトリで初回のみ行い、作成し直したときは再度行うこと
    #[test]
    fn should_restrict_config_dir_only_once_unless_recreated() {
        let dir = PathBuf::from(format!("clip-refiner-restrict-once-{}", std::process::id()));

        // 初回は設定対象、2 回目以降は対象外
        assert!(should_restrict_config_dir(&dir, false));
        assert!(!should_restrict_config_dir(&dir, false));

        // 削除後に作成し直した場合は再設定する
        assert!(should_restrict_config_dir(&dir, true));
        assert!(!should_restrict_config_dir(&dir, false));

        // 別ディレクトリは独立して初回扱いとする
        assert!(should_restrict_config_dir(&dir.join("other"), false));
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
        assert!(!portable_mode_with(Some(Path::new("env-config-dir"))));
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
