//! インストール済みアプリの状態 (実行ファイルの版・自動起動の登録)

use std::ffi::c_void;
use std::io;
use std::path::Path;

use clip_refiner_core::consts::APP_NAME;
use semver::Version;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};
use windows::core::{HSTRING, w};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, RegValue};

use super::replace::portable_exe_name;

// ======================================================================
// 定数
// ======================================================================
/// ログイン時の自動起動を登録するレジストリキー (`HKCU`)
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

// ======================================================================
// 版
// ======================================================================
/// インストール先の `ClipRefiner.exe` の版を返す
///
/// プレリリースの識別子はファイルの版情報に含まれないため、数値部分だけになる
///
/// # Arguments
/// * `install_dir` - `ClipRefiner.exe` のあるフォルダ
///
/// # Returns
/// * `Option<Version>` - 版情報を読めない場合は `None`
pub(crate) fn installed_version(install_dir: &Path) -> Option<Version> {
    file_version(&install_dir.join(portable_exe_name()))
}

/// 数値部分 (メジャー・マイナー・パッチ) が一致するか
///
/// # Arguments
/// * `installed` - インストール済みの版
/// * `expected` - 更新先の版 (`SemVer` 文字列)
///
/// # Returns
/// * `bool` - `expected` を解釈でき、数値部分が一致する場合は `true`
pub(crate) fn matches_version(installed: &Version, expected: &str) -> bool {
    Version::parse(expected).is_ok_and(|expected| {
        (installed.major, installed.minor, installed.patch)
            == (expected.major, expected.minor, expected.patch)
    })
}

/// 実行ファイルの版情報を読む
fn file_version(path: &Path) -> Option<Version> {
    let path = HSTRING::from(path.as_os_str());
    // SAFETY: `path` は NUL 終端の UTF-16 で、呼び出し完了まで生存する
    let size = unsafe { GetFileVersionInfoSizeW(&path, None) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: `data` は `size` バイトの書き込み可能な領域で、`path` は呼び出し完了まで生存する
    unsafe { GetFileVersionInfoW(&path, None, size, data.as_mut_ptr().cast::<c_void>()) }.ok()?;

    let mut info: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `data` は `GetFileVersionInfoW` が書き込んだ版情報で、`info` は `data` の内部を指す
    let found = unsafe {
        VerQueryValueW(
            data.as_ptr().cast::<c_void>(),
            w!("\\"),
            &raw mut info,
            &raw mut len,
        )
    };
    if !found.as_bool() || usize::try_from(len).ok()? < size_of::<VS_FIXEDFILEINFO>() {
        return None;
    }
    // SAFETY: `info` は `data` 内の `VS_FIXEDFILEINFO` を指し、長さは直前に確かめた
    // `data` のアラインメントは保証されないため、参照を作らずに読み取る
    let fixed = unsafe { info.cast::<VS_FIXEDFILEINFO>().read_unaligned() };
    Some(Version::new(
        u64::from(fixed.dwFileVersionMS >> 16),
        u64::from(fixed.dwFileVersionMS & 0xFFFF),
        u64::from(fixed.dwFileVersionLS >> 16),
    ))
}

// ======================================================================
// 自動起動
// ======================================================================
/// 更新前の自動起動の登録
///
/// MSI は更新時に自動起動を登録し直すため、利用者が設定で変えた状態を控えて戻す
pub(crate) struct AutostartSnapshot {
    /// 登録が無ければ `None`
    value: Option<RegValue<'static>>,
}

impl AutostartSnapshot {
    /// 現在の登録を控える
    ///
    /// # Returns
    /// * `Option<Self>` - 登録の有無を読み取れない場合は `None`
    pub(crate) fn capture() -> Option<Self> {
        Self::capture_from(RUN_KEY)
    }

    /// 控えた登録へ戻す
    pub(crate) fn restore(&self) {
        if let Err(err) = self.restore_to(RUN_KEY) {
            tracing::warn!("自動起動の登録を更新前の状態へ戻せない: {err}");
        }
    }

    /// 指定したキーから控える
    fn capture_from(key_path: &str) -> Option<Self> {
        let result = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(key_path, KEY_READ)
            .and_then(|key| key.get_raw_value(APP_NAME));
        match result {
            Ok(value) => Some(Self { value: Some(value) }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Some(Self { value: None }),
            Err(err) => {
                tracing::warn!("自動起動の登録を読み取れない: {err}");
                None
            }
        }
    }

    /// 指定したキーへ戻す
    fn restore_to(&self, key_path: &str) -> io::Result<()> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(key_path, KEY_READ | KEY_SET_VALUE)?;
        match &self.value {
            Some(value) => key.set_raw_value(APP_NAME, value),
            None => match key.delete_value(APP_NAME) {
                Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            },
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用のレジストリキー。破棄時に削除する
    struct TestKey {
        path: String,
        key: RegKey,
    }

    impl TestKey {
        fn create(label: &str) -> Self {
            let path = format!(r"Software\ClipRefiner-Test-{label}-{}", std::process::id());
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(&path)
                .expect("create key");
            Self { path, key }
        }
    }

    impl Drop for TestKey {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.path);
        }
    }

    /// 版情報を持つ実行ファイルの版を読めること
    #[test]
    fn file_version_reads_system_exe() {
        let path = crate::system_exe_path("msiexec.exe").expect("path");
        let version = file_version(&path).expect("version");
        assert!(version.major >= 5, "{version}");
    }

    /// 実行ファイルが無いときは版を返さないこと
    #[test]
    fn installed_version_is_none_without_exe() {
        assert!(installed_version(Path::new("missing-install-dir")).is_none());
    }

    /// 版の数値部分だけを比べること
    #[test]
    fn matches_version_compares_numeric_parts() {
        let installed = Version::new(4, 1, 0);
        assert!(matches_version(&installed, "4.1.0"));
        assert!(matches_version(&installed, "4.1.0-rc.1"));
        assert!(!matches_version(&installed, "4.1.1"));
        assert!(!matches_version(&installed, "不正"));
    }

    /// 自動起動が登録されていた場合は、更新中に変わっても元の値へ戻すこと
    #[test]
    fn autostart_snapshot_restores_value() {
        let test = TestKey::create("autostart-value");
        test.key
            .set_value(APP_NAME, &"before".to_string())
            .expect("set");

        let snapshot = AutostartSnapshot::capture_from(&test.path).expect("capture");
        test.key
            .set_value(APP_NAME, &"after".to_string())
            .expect("overwrite");
        snapshot.restore_to(&test.path).expect("restore");

        let value: String = test.key.get_value(APP_NAME).expect("get");
        assert_eq!(value, "before");
    }

    /// 自動起動が登録されていなかった場合は、更新で追加された登録を取り除くこと
    #[test]
    fn autostart_snapshot_removes_added_value() {
        let test = TestKey::create("autostart-none");
        let snapshot = AutostartSnapshot::capture_from(&test.path).expect("capture");
        test.key
            .set_value(APP_NAME, &"added".to_string())
            .expect("add");

        snapshot.restore_to(&test.path).expect("restore");
        assert!(test.key.get_value::<String, _>(APP_NAME).is_err());
        // 登録が無いままでも失敗しない
        snapshot.restore_to(&test.path).expect("restore again");
    }
}
