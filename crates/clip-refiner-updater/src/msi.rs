//! MSI によるインストール
//!
//! インストーラーの画面は出さない (`/qn`)。進捗と結果は更新プログラムの画面に表示する

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::ApplicationInstallationAndServicing::{
    MsiEnumRelatedProductsW, MsiGetProductInfoW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

use super::error::UpdateError;
use super::system_exe_path;
use super::types::InstallOutcome;

// ======================================================================
// 定数
// ======================================================================
/// MSI の `UpgradeCode` (`wix/main.wxs` と揃える)
const UPGRADE_CODE: &str = "{A3C8E2F1-9B4D-4E7A-8C1F-2D5E6A9B0C3D}";
/// インストール先 (`APPLICATIONFOLDER`) 直下の、実行ファイルを置くフォルダ名
const BIN_DIR_NAME: &str = "bin";
/// `ProductCode` の文字数 (GUID 38 文字 + NUL)
const PRODUCT_CODE_CHARS: usize = 39;
/// インストール先パスの取得バッファ (UTF-16 単位)
const INSTALL_LOCATION_CHARS: usize = 32_768;
/// 成功
const EXIT_SUCCESS: i32 = 0;
/// 別のインストールが進行中
const EXIT_INSTALL_ALREADY_RUNNING: i32 = 1618;
/// 成功 (再起動を開始した)
const EXIT_REBOOT_INITIATED: i32 = 1641;
/// 成功 (再起動が必要)
const EXIT_REBOOT_REQUIRED: i32 = 3010;

// ======================================================================
// インストール先
// ======================================================================
/// 実行フォルダが MSI の導入先か確かめ、`APPLICATIONFOLDER` を返す
///
/// 別のフォルダへ複製した `ClipRefiner.exe` から更新すると、
/// 導入先だけが新しくなり、動かしていた方は古いまま残るため
///
/// # Arguments
/// * `install_dir` - `ClipRefiner.exe` のあるフォルダ
///
/// # Returns
/// * `Result<PathBuf, UpdateError>` - インストール先 (`bin` の親フォルダ)
pub(crate) fn application_folder(install_dir: &Path) -> Result<PathBuf, UpdateError> {
    find_application_folder(install_dir, &installed_locations()).ok_or_else(|| {
        tracing::warn!("MSI の導入先ではない: {}", install_dir.display());
        UpdateError::InstallLocationMismatch
    })
}

/// 導入先の一覧から、`bin` が実行フォルダと一致するものを返す
fn find_application_folder(install_dir: &Path, locations: &[PathBuf]) -> Option<PathBuf> {
    let install = install_dir.canonicalize().ok()?;
    locations
        .iter()
        .find(|location| {
            location.join(BIN_DIR_NAME).canonicalize().is_ok_and(|bin| {
                bin.to_string_lossy()
                    .eq_ignore_ascii_case(&install.to_string_lossy())
            })
        })
        .cloned()
}

/// 導入済みの `ClipRefiner` のインストール先を列挙する
fn installed_locations() -> Vec<PathBuf> {
    let upgrade_code = HSTRING::from(UPGRADE_CODE);
    let mut locations = Vec::new();
    for index in 0.. {
        let mut product = [0u16; PRODUCT_CODE_CHARS];
        // SAFETY: `product` は NUL を含む 39 文字の領域で、`upgrade_code` は呼び出し完了まで生存する
        let result = unsafe {
            MsiEnumRelatedProductsW(&upgrade_code, None, index, PWSTR(product.as_mut_ptr()))
        };
        if result != ERROR_SUCCESS.0 {
            break;
        }
        if let Some(location) = install_location(&product) {
            locations.push(location);
        }
    }
    locations
}

/// 製品のインストール先 (`ARPINSTALLLOCATION`) を返す
fn install_location(product: &[u16; PRODUCT_CODE_CHARS]) -> Option<PathBuf> {
    let mut buf = vec![0u16; INSTALL_LOCATION_CHARS];
    let mut len = u32::try_from(buf.len()).ok()?;
    // SAFETY: `product` は NUL 終端の GUID、`buf` は `len` 文字の領域で、いずれも呼び出し完了まで生存する
    let result = unsafe {
        MsiGetProductInfoW(
            PCWSTR(product.as_ptr()),
            w!("InstallLocation"),
            Some(PWSTR(buf.as_mut_ptr())),
            Some(&raw mut len),
        )
    };
    let len = usize::try_from(len).ok()?;
    if result != ERROR_SUCCESS.0 || len == 0 || len > buf.len() {
        return None;
    }
    // 末尾の区切りは `msiexec` の引数で閉じ引用符と紛れるため外す
    let raw = String::from_utf16(&buf[..len]).ok()?;
    Some(PathBuf::from(raw.trim_end_matches('\\')))
}

// ======================================================================
// インストール
// ======================================================================
/// MSI をインストールし、終わるまで待つ
///
/// # Arguments
/// * `msi_path` - 検証済みの MSI
/// * `application_folder` - 維持するインストール先
/// * `log_path` - `msiexec` の詳細ログの出力先
///
/// # Returns
/// * `Result<InstallOutcome, UpdateError>` - インストール結果
pub(crate) fn install_msi(
    msi_path: &Path,
    application_folder: &Path,
    log_path: &Path,
) -> Result<InstallOutcome, UpdateError> {
    let Some(msiexec) = system_exe_path("msiexec.exe") else {
        tracing::warn!("システムフォルダの取得に失敗");
        return Err(UpdateError::MsiLaunch);
    };
    let args = msiexec_args(msi_path, application_folder, log_path)?;
    let status = Command::new(msiexec)
        .raw_arg(args)
        .status()
        .map_err(|err| {
            tracing::warn!("msiexec の起動に失敗: {err}");
            UpdateError::MsiLaunch
        })?;
    tracing::info!("msiexec が終了しました: {status}");
    outcome_from_exit_code(status.code().unwrap_or(-1))
}

/// `msiexec` の引数を組み立てる
///
/// `msiexec` は `"NAME=値"` 形式を解釈しないため、値だけを引用符で囲む
fn msiexec_args(
    msi_path: &Path,
    application_folder: &Path,
    log_path: &Path,
) -> Result<String, UpdateError> {
    let quote = |path: &Path| match path.to_str() {
        Some(path) if !path.contains('"') => Ok(format!("\"{path}\"")),
        _ => Err(UpdateError::PathInvalid),
    };
    Ok(format!(
        "/i {} /qn /norestart APPLICATIONFOLDER={} /L*v {}",
        quote(msi_path)?,
        quote(application_folder)?,
        quote(log_path)?
    ))
}

/// `msiexec` の終了コードをインストール結果へ変換する
fn outcome_from_exit_code(code: i32) -> Result<InstallOutcome, UpdateError> {
    match code {
        EXIT_SUCCESS => Ok(InstallOutcome::Installed),
        EXIT_REBOOT_REQUIRED | EXIT_REBOOT_INITIATED => Ok(InstallOutcome::RebootRequired),
        EXIT_INSTALL_ALREADY_RUNNING => Err(UpdateError::MsiBusy),
        code => Err(UpdateError::MsiFailed { code }),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// `UpgradeCode` が `WiX` の定義と一致すること
    #[test]
    fn upgrade_code_matches_wix() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wix/main.wxs");
        let wxs = fs::read_to_string(path).expect("main.wxs");
        let code = UPGRADE_CODE.trim_matches(['{', '}']);
        assert!(wxs.contains(&format!("UpgradeCode='{code}'")));
    }

    /// 空白を含むパスでも、値だけを引用符で囲むこと
    #[test]
    fn msiexec_args_quote_only_values() {
        let args = msiexec_args(
            Path::new(r"C:\Temp dir\app.msi"),
            Path::new(r"D:\My Apps\ClipRefiner"),
            Path::new(r"C:\logs\updater-msi.log"),
        );
        assert_eq!(
            args.as_deref(),
            Ok(concat!(
                r#"/i "C:\Temp dir\app.msi" /qn /norestart "#,
                r#"APPLICATIONFOLDER="D:\My Apps\ClipRefiner" "#,
                r#"/L*v "C:\logs\updater-msi.log""#
            ))
        );
    }

    /// 引用符を含むパスを拒否すること
    #[test]
    fn msiexec_args_reject_quote() {
        let result = msiexec_args(
            Path::new("a\" /x b.msi"),
            Path::new("app"),
            Path::new("log"),
        );
        assert_eq!(result, Err(UpdateError::PathInvalid));
    }

    /// 終了コードから成功・再起動待ち・失敗を区別すること
    #[test]
    fn exit_codes_map_to_outcome() {
        assert_eq!(outcome_from_exit_code(0), Ok(InstallOutcome::Installed));
        for code in [3010, 1641] {
            assert_eq!(
                outcome_from_exit_code(code),
                Ok(InstallOutcome::RebootRequired)
            );
        }
        assert_eq!(outcome_from_exit_code(1618), Err(UpdateError::MsiBusy));
        assert_eq!(
            outcome_from_exit_code(1603),
            Err(UpdateError::MsiFailed { code: 1603 })
        );
    }

    /// 実行フォルダが導入先の `bin` と一致する場合だけ、導入先を返すこと
    #[test]
    fn application_folder_requires_matching_bin() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-msi-location-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let app = root.join("ClipRefiner");
        let copied = root.join("copied");
        fs::create_dir_all(app.join("bin")).expect("bin");
        fs::create_dir_all(&copied).expect("copied");
        let locations = [root.join("other"), app.clone()];

        assert_eq!(
            find_application_folder(&app.join("bin"), &locations),
            Some(app.clone())
        );
        assert_eq!(find_application_folder(&copied, &locations), None);
        assert_eq!(find_application_folder(&app.join("bin"), &[]), None);

        let _ = fs::remove_dir_all(&root);
    }
}
