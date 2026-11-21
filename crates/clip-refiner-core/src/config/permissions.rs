//! 設定・ログディレクトリと名前付きパイプのアクセス制限
//!
//! Windows では現在ユーザー専用 DACL を設定する

use std::fs;
use std::mem::size_of;
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
    ACE_FLAGS, ACL, CopySid, DACL_SECURITY_INFORMATION, GetLengthSid, GetTokenInformation,
    InitializeSecurityDescriptor, NO_INHERITANCE, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    SUB_CONTAINERS_AND_OBJECTS_INHERIT, SetSecurityDescriptorDacl, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

/// `InitializeSecurityDescriptor` のリビジョン (`SECURITY_DESCRIPTOR_REVISION`)
const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

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
// 名前付きパイプ向け SECURITY_ATTRIBUTES
// ======================================================================
/// 現在ユーザー専用 DACL を持つ `SECURITY_ATTRIBUTES`
///
/// `CreateNamedPipeW` などに渡し、呼び出し完了までドロップしない
pub struct OwnerOnlySecurityAttributes {
    /// ヒープ上の記述子 (`attrs` が指すため `Box` で移動してもポインタが安定)
    _descriptor: Box<SECURITY_DESCRIPTOR>,
    /// Win32 API へ渡す属性
    attrs: SECURITY_ATTRIBUTES,
    /// `SetEntriesInAclW` が確保した ACL (`Drop` で `LocalFree`)
    dacl: *mut ACL,
}

// `dacl` の生ポインタは `LocalFree` 専用で、スレッド間共有しない
unsafe impl Send for OwnerOnlySecurityAttributes {}

impl OwnerOnlySecurityAttributes {
    /// 現在ユーザーのみへ `GENERIC_ALL` を付与した属性を構築する
    ///
    /// # Returns
    /// * `Result<Self>` - 構築した属性
    ///
    /// # Errors
    /// SID 取得・ACL / セキュリティ記述子の構築に失敗した場合
    pub fn new() -> Result<Self> {
        let dacl = build_owner_only_acl(NO_INHERITANCE)?;
        let mut descriptor = Box::new(SECURITY_DESCRIPTOR::default());
        let descriptor_ptr = ptr::from_mut(descriptor.as_mut()).cast();

        unsafe {
            InitializeSecurityDescriptor(
                PSECURITY_DESCRIPTOR(descriptor_ptr),
                SECURITY_DESCRIPTOR_REVISION,
            )
            .context("セキュリティ記述子の初期化に失敗しました")?;

            SetSecurityDescriptorDacl(
                PSECURITY_DESCRIPTOR(descriptor_ptr),
                true,
                Some(dacl),
                false,
            )
            .context("セキュリティ記述子への DACL 設定に失敗しました")?;
        }

        let n_length = u32::try_from(size_of::<SECURITY_ATTRIBUTES>())
            .context("SECURITY_ATTRIBUTES のサイズが u32 に収まりません")?;
        let attrs = SECURITY_ATTRIBUTES {
            nLength: n_length,
            lpSecurityDescriptor: descriptor_ptr,
            bInheritHandle: false.into(),
        };

        Ok(Self {
            _descriptor: descriptor,
            attrs,
            dacl,
        })
    }

    /// `CreateNamedPipeW` などへ渡すポインタを返す
    ///
    /// # Arguments
    /// * `self` - この属性
    ///
    /// # Returns
    /// * `*const SECURITY_ATTRIBUTES` - 属性へのポインタ
    pub fn as_ptr(&self) -> *const SECURITY_ATTRIBUTES {
        &raw const self.attrs
    }
}

impl Drop for OwnerOnlySecurityAttributes {
    fn drop(&mut self) {
        if !self.dacl.is_null() {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.dacl.cast())));
            }
            self.dacl = ptr::null_mut();
        }
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// パスを現在のユーザー専用 DACL に置き換える
fn apply_owner_only_dacl(path: &Path, is_directory: bool) -> Result<()> {
    let wide_path = path_to_wide(path);

    let inheritance = if is_directory {
        SUB_CONTAINERS_AND_OBJECTS_INHERIT
    } else {
        NO_INHERITANCE
    };

    let new_dacl = build_owner_only_acl(inheritance)?;

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

/// 現在ユーザーへ `GENERIC_ALL` を付与する ACL を構築する
///
/// 戻り値は呼び出し側が `LocalFree` する
///
/// # Arguments
/// * `inheritance` - ACE の継承フラグ
///
/// # Returns
/// * `Result<*mut ACL>` - 構築した ACL
///
/// # Errors
/// SID 取得または `SetEntriesInAclW` に失敗した場合
fn build_owner_only_acl(inheritance: ACE_FLAGS) -> Result<*mut ACL> {
    let mut sid = current_process_user_sid()?;
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

    Ok(new_dacl)
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

    /// 現在ユーザー専用の `SECURITY_ATTRIBUTES` を構築できるようにすること
    #[test]
    fn owner_only_security_attributes_builds() {
        let attrs = OwnerOnlySecurityAttributes::new().expect("OwnerOnlySecurityAttributes::new");
        assert!(!attrs.as_ptr().is_null());
        assert_eq!(
            unsafe { (*attrs.as_ptr()).nLength },
            u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).expect("size fits u32")
        );
        assert!(!unsafe { (*attrs.as_ptr()).lpSecurityDescriptor.is_null() });
    }
}
