//! 設定・ログディレクトリのアクセス制限
//!
//! Windows では現在ユーザー専用 DACL を設定する

use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{
    CloseHandle, GENERIC_ALL, GetLastError, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, SetEntriesInAclW,
    SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
};
use windows::Win32::Security::{
    CopySid, DACL_SECURITY_INFORMATION, GetLengthSid, GetTokenInformation, NO_INHERITANCE,
    PROTECTED_DACL_SECURITY_INFORMATION, PSID, SUB_CONTAINERS_AND_OBJECTS_INHERIT, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

// ======================================================================
// パブリック関数
// ======================================================================
/// ディレクトリを所有者のみアクセス可能にする
///
/// 子オブジェクトへ継承する DACL を設定する
///
/// # Arguments
/// * `path` - パーミッションを設定するディレクトリパス
///
/// # Returns
/// * `Result<()>` - 設定に成功した場合は `Ok(())`
///
/// # Errors
/// DACL の設定に失敗した場合
pub fn restrict_private_dir_permissions(path: &Path) -> Result<()> {
    apply_owner_only_dacl(path, true)
}

/// ファイルを所有者のみ読み書き可能にする
pub(crate) fn restrict_private_file_permissions(path: &Path) -> Result<()> {
    apply_owner_only_dacl(path, false)
}

/// ディレクトリ内の既存ファイルへ所有者専用 ACL を適用する
///
/// # Arguments
/// * `dir` - 走査対象のディレクトリパス
///
/// # Returns
/// * `Result<()>` - 全ファイルへの適用に成功した場合は `Ok(())`
///
/// # Errors
/// ディレクトリ走査または個別ファイルのパーミッション設定に失敗した場合
pub fn restrict_private_files_in_dir(dir: &Path) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| {
        format!(
            "ディレクトリ内ファイルのパーミッション設定に失敗: {}",
            dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "ディレクトリ内ファイルのパーミッション設定に失敗: {}",
                dir.display()
            )
        })?;
        let path = entry.path();
        if path.is_file() {
            restrict_private_file_permissions(&path)?;
        }
    }

    Ok(())
}

// ======================================================================
// プライベート関数
// ======================================================================
/// パスを現在のユーザー専用 DACL に置き換える
fn apply_owner_only_dacl(path: &Path, is_directory: bool) -> Result<()> {
    let sid = current_process_user_sid()?;
    let wide_path = path_to_wide(path);

    let inheritance = if is_directory {
        SUB_CONTAINERS_AND_OBJECTS_INHERIT
    } else {
        NO_INHERITANCE
    };

    let mut sid = sid;
    let trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        // SID バッファは `SetEntriesInAclW` 呼び出しまで生存させる
        ptstrName: PWSTR(sid.as_mut_ptr().cast()),
    };

    let explicit = EXPLICIT_ACCESS_W {
        grfAccessPermissions: GENERIC_ALL.0,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: inheritance,
        Trustee: trustee,
    };

    let mut new_dacl = ptr::null_mut();
    let set_acl_result = unsafe { SetEntriesInAclW(Some(&[explicit]), None, &raw mut new_dacl) };
    if !set_acl_result.is_ok() {
        return Err(anyhow::anyhow!(
            "DACL の構築に失敗しました (code={})",
            set_acl_result.0
        ));
    }

    let security_info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
    let set_security_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide_path.as_ptr()),
            SE_FILE_OBJECT,
            security_info,
            None,
            None,
            Some(new_dacl),
            None,
        )
    };

    unsafe {
        let _ = LocalFree(Some(HLOCAL(new_dacl.cast())));
    }

    if !set_security_result.is_ok() {
        return Err(anyhow::anyhow!(
            "ファイルのセキュリティ設定に失敗しました: {} (code={})",
            path.display(),
            set_security_result.0
        ));
    }

    Ok(())
}

/// 現在のプロセスを実行しているユーザーの SID を返す
fn current_process_user_sid() -> Result<Vec<u8>> {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token).is_err() {
            return Err(anyhow::anyhow!(
                "プロセストークンのオープンに失敗しました (code={})",
                GetLastError().0
            ));
        }

        let mut buffer_len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &raw mut buffer_len);

        let mut buffer = vec![0u8; buffer_len as usize];
        if GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            buffer_len,
            &raw mut buffer_len,
        )
        .is_err()
        {
            let code = GetLastError().0;
            let _ = CloseHandle(token);
            return Err(anyhow::anyhow!(
                "トークン情報の取得に失敗しました (code={code})"
            ));
        }
        let _ = CloseHandle(token);

        let token_user = ptr::read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>());
        let sid = token_user.User.Sid;
        let sid_len = GetLengthSid(sid);
        let mut sid_copy = vec![0u8; sid_len as usize];
        if CopySid(sid_len, PSID(sid_copy.as_mut_ptr().cast()), sid).is_err() {
            return Err(anyhow::anyhow!(
                "SID のコピーに失敗しました (code={})",
                GetLastError().0
            ));
        }

        Ok(sid_copy)
    }
}

/// パスをヌル終端の UTF-16 に変換する
fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// 一時ディレクトリへ所有者専用 DACL を設定できるようにすること
    #[test]
    fn restrict_private_dir_permissions_succeeds_on_temp_dir() {
        let dir =
            std::env::temp_dir().join(format!("clip-refiner-perm-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");

        let result = restrict_private_dir_permissions(&dir);
        let _ = fs::remove_dir_all(&dir);

        result.expect("restrict_private_dir_permissions");
    }

    /// 一時ファイルへ所有者専用 ACL を設定できるようにすること
    #[test]
    fn restrict_private_file_permissions_succeeds_on_temp_file() {
        let dir = std::env::temp_dir().join(format!(
            "clip-refiner-file-perm-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let file = dir.join("secret.txt");
        fs::write(&file, b"test").expect("write temp file");

        let result = restrict_private_file_permissions(&file);
        let _ = fs::remove_dir_all(&dir);

        result.expect("restrict_private_file_permissions");
    }
}
