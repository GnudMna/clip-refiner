//! 設定・ログディレクトリのアクセス制限
//!
//! Unix では `chmod`、Windows では現在ユーザー専用 DACL を設定する

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

// ======================================================================
// パブリック関数
// ======================================================================
/// ディレクトリを所有者のみアクセス可能にする
///
/// Windows では子オブジェクトへ継承する DACL を設定する
pub(crate) fn restrict_private_dir_permissions(path: &Path) -> Result<()> {
    platform::restrict_private_dir_permissions(path)
}

/// ファイルを所有者のみ読み書き可能にする
pub(crate) fn restrict_private_file_permissions(path: &Path) -> Result<()> {
    platform::restrict_private_file_permissions(path)
}

/// ディレクトリ内の既存ファイルへ所有者専用 ACL / パーミッションを適用する
pub(crate) fn restrict_private_files_in_dir(dir: &Path) -> Result<()> {
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
// Unix
// ======================================================================
#[cfg(unix)]
mod platform {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use anyhow::{Context, Result};

    /// ディレクトリを所有者のみアクセス可能にする
    pub(super) fn restrict_private_dir_permissions(path: &Path) -> Result<()> {
        let mut perms = fs::metadata(path)
            .context("設定ディレクトリのメタデータ取得に失敗しました")?
            .permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)
            .context("設定ディレクトリのパーミッション設定に失敗しました")?;
        Ok(())
    }

    /// ファイルを所有者のみ読み書き可能にする
    pub(super) fn restrict_private_file_permissions(path: &Path) -> Result<()> {
        let mut perms = fs::metadata(path)
            .context("設定ファイルのメタデータ取得に失敗しました")?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)
            .context("設定ファイルのパーミッション設定に失敗しました")?;
        Ok(())
    }
}

// ======================================================================
// Windows
// ======================================================================
#[cfg(windows)]
mod platform {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use anyhow::Result;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_ALL, GetLastError, HANDLE, LocalFree,
    };
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, SetEntriesInAclW,
        SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        CopySid, DACL_SECURITY_INFORMATION, GetLengthSid, GetTokenInformation, NO_INHERITANCE,
        PROTECTED_DACL_SECURITY_INFORMATION, SUB_CONTAINERS_AND_OBJECTS_INHERIT, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// ディレクトリを所有者のみアクセス可能にする
    pub(super) fn restrict_private_dir_permissions(path: &Path) -> Result<()> {
        apply_owner_only_dacl(path, true)
    }

    /// ファイルを所有者のみ読み書き可能にする
    pub(super) fn restrict_private_file_permissions(path: &Path) -> Result<()> {
        apply_owner_only_dacl(path, false)
    }

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
            ptstrName: sid.as_mut_ptr().cast(),
        };

        let explicit = EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_ALL,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: inheritance,
            Trustee: trustee,
        };

        let mut new_dacl = ptr::null_mut();
        let set_acl_result =
            unsafe { SetEntriesInAclW(1, &raw const explicit, ptr::null(), &raw mut new_dacl) };
        if set_acl_result != 0 {
            return Err(anyhow::anyhow!(
                "DACL の構築に失敗しました (code={set_acl_result})"
            ));
        }

        let security_info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
        let set_security_result = unsafe {
            SetNamedSecurityInfoW(
                wide_path.as_ptr().cast_mut(),
                SE_FILE_OBJECT,
                security_info,
                ptr::null_mut(),
                ptr::null_mut(),
                new_dacl,
                ptr::null_mut(),
            )
        };

        unsafe {
            LocalFree(new_dacl.cast());
        }

        if set_security_result != 0 {
            return Err(anyhow::anyhow!(
                "ファイルのセキュリティ設定に失敗しました: {} (code={set_security_result})",
                path.display()
            ));
        }

        Ok(())
    }

    /// 現在のプロセスを実行しているユーザーの SID を返す
    fn current_process_user_sid() -> Result<Vec<u8>> {
        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) == 0 {
                return Err(anyhow::anyhow!(
                    "プロセストークンのオープンに失敗しました (code={})",
                    GetLastError()
                ));
            }

            let mut buffer_len = 0u32;
            let _ = GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &raw mut buffer_len);

            let mut buffer = vec![0u8; buffer_len as usize];
            if GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                buffer_len,
                &raw mut buffer_len,
            ) == 0
            {
                let code = GetLastError();
                CloseHandle(token);
                return Err(anyhow::anyhow!(
                    "トークン情報の取得に失敗しました (code={code})"
                ));
            }
            CloseHandle(token);

            let token_user = ptr::read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>());
            let sid = token_user.User.Sid;
            let sid_len = GetLengthSid(sid);
            let mut sid_copy = vec![0u8; sid_len as usize];
            if CopySid(sid_len, sid_copy.as_mut_ptr().cast(), sid) == 0 {
                return Err(anyhow::anyhow!(
                    "SID のコピーに失敗しました (code={})",
                    GetLastError()
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
}

// ======================================================================
// その他プラットフォーム
// ======================================================================
#[cfg(not(any(unix, windows)))]
mod platform {
    use std::path::Path;

    use anyhow::Result;

    /// ディレクトリを所有者のみアクセス可能にする
    pub(super) fn restrict_private_dir_permissions(_path: &Path) -> Result<()> {
        Ok(())
    }

    /// ファイルを所有者のみ読み書き可能にする
    pub(super) fn restrict_private_file_permissions(_path: &Path) -> Result<()> {
        Ok(())
    }
}
