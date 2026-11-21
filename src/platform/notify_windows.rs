use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Once;

use super::notify::format_notification_summary;

use crate::consts;

use anyhow::{Context, Result};
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::core::HSTRING;
use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use winreg::RegKey;
use winreg::RegValue;
use winreg::enums::{HKEY_CURRENT_USER, REG_EXPAND_SZ};

/// `AppUserModelID` を登録するレジストリキー (`HKCU\Software\Classes\AppUserModelId\<APP_ID>`)
const AUMID_REGISTRY_KEY_PREFIX: &str = r"Software\Classes\AppUserModelId";

/// 設定ディレクトリへ書き出す送信元アイコンのファイル名
const REGISTRY_ICON_FILE: &str = "notification-icon.ico";

/// 送信元アイコンとして書き出す埋め込み ICO
const REGISTRY_ICON_BYTES: &[u8] = include_bytes!("../../assets/icon.ico");

static INIT: Once = Once::new();

/// Windows トースト通知向けに `AppUserModelID` を初期化する
///
/// プロセスに AUMID を設定し、通知センター表示名とアイコンをレジストリへ登録する
/// 失敗時はログへ記録するが、起動処理は継続する
pub fn init_notifications() {
    ensure_initialized();
}

/// システム通知を表示する
///
/// OSの通知機能を使用して、デスクトップ上にメッセージを表示する
/// 通知は約3秒後に自動的に消える
/// 表示に失敗した場合はログへ記録する
///
/// # Arguments
/// * `summary` - 通知のタイトル(「ClipRefiner - タイトル」の形式で表示される)
/// * `body` - 通知の本文
pub fn show_notification(summary: &str, body: &str) {
    ensure_initialized();

    if let Err(e) = show_toast(consts::APP_ID, &format_notification_summary(summary), body) {
        crate::log_warn!("通知の表示に失敗: {:?}", e);
    }
}

/// `AppUserModelID` の初期化を一度だけ実行する
fn ensure_initialized() {
    INIT.call_once(|| {
        if let Err(e) = init_notifications_inner() {
            crate::log_warn!("通知の初期化に失敗: {:?}", e);
        }
    });
}

/// プロセスとレジストリへ `AppUserModelID` を登録する
fn init_notifications_inner() -> Result<()> {
    set_process_app_user_model_id(consts::APP_ID)?;
    let icon_path = materialize_registry_icon()?;
    register_app_user_model_id(consts::APP_NAME, &icon_path)?;
    Ok(())
}

/// 実行中プロセスへ `AppUserModelID` を設定する
fn set_process_app_user_model_id(app_id: &str) -> Result<()> {
    let app_id_wide = to_wide(app_id);
    // SAFETY: `app_id_wide` はヌル終端の有効な UTF-16 文字列
    let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(app_id_wide.as_ptr()) };
    if hr < 0 {
        anyhow::bail!("SetCurrentProcessExplicitAppUserModelID が失敗: HRESULT {hr:#x}");
    }
    Ok(())
}

/// 通知センター表示用の `AppUserModelID` 情報をレジストリへ登録する
fn register_app_user_model_id(display_name: &str, icon_uri: &Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key_path = aumid_registry_key_path(consts::APP_ID);
    let (key, _) = hkcu
        .create_subkey(&key_path)
        .with_context(|| format!("AppUserModelID レジストリキーの作成に失敗: {key_path}"))?;

    key.set_raw_value("DisplayName", &to_reg_expand_sz(display_name))
        .context("DisplayName の登録に失敗")?;
    key.set_raw_value("IconUri", &to_reg_expand_sz(&path_for_registry(icon_uri)))
        .context("IconUri の登録に失敗")?;
    key.set_raw_value("IconBackgroundColor", &to_reg_expand_sz("0"))
        .context("IconBackgroundColor の登録に失敗")?;

    Ok(())
}

/// 埋め込み ICO を設定ディレクトリへ書き出し、送信元アイコンのパスを返す
fn materialize_registry_icon() -> Result<PathBuf> {
    let config_dir = crate::config::get_config_dir()?;
    std::fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗")?;

    let icon_path = config_dir.join(REGISTRY_ICON_FILE);
    std::fs::write(&icon_path, REGISTRY_ICON_BYTES)
        .with_context(|| format!("送信元アイコンの書き出しに失敗: {}", icon_path.display()))?;

    Ok(icon_path)
}

/// `WinRT` のトーストを表示する
///
/// 上部の送信元アイコンはレジストリ `IconUri` から表示される
fn show_toast(app_id: &str, title: &str, body: &str) -> Result<()> {
    let title = xml_escape(title);
    let body = xml_escape(body);

    let xml = format!(
        r#"<toast duration="short">
            <visual>
                <binding template="ToastGeneric">
                    <text id="1">{title}</text>
                    <text id="3">{body}</text>
                </binding>
            </visual>
        </toast>"#
    );

    let toast_xml = XmlDocument::new().context("トースト XML ドキュメントの作成に失敗")?;
    toast_xml
        .LoadXml(&HSTRING::from(xml))
        .context("トースト XML の読み込みに失敗")?;

    let toast = ToastNotification::CreateToastNotification(&toast_xml)
        .context("トースト通知オブジェクトの作成に失敗")?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))
        .context("トースト通知 Notifier の作成に失敗")?;
    notifier.Show(&toast).context("トースト通知の表示に失敗")?;

    Ok(())
}

/// レジストリ文字列 (`REG_EXPAND_SZ`) を組み立てる
fn to_reg_expand_sz(value: &str) -> RegValue<'static> {
    let wide: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    let bytes = wide
        .iter()
        .flat_map(|code_unit| code_unit.to_le_bytes())
        .collect::<Vec<u8>>();

    RegValue {
        bytes: bytes.into(),
        vtype: REG_EXPAND_SZ,
    }
}

/// レジストリ `IconUri` 向けの通常パス文字列を返す
fn path_for_registry(path: &Path) -> String {
    strip_verbatim_prefix(path)
}

/// `canonicalize` 由来の `\\?\` プレフィックスを除去したパス文字列を返す
fn strip_verbatim_prefix(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    path_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&path_str)
        .to_owned()
}

/// トースト XML テキスト向けに特殊文字をエスケープする
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// `AppUserModelID` 用レジストリキーの相対パスを組み立てる
fn aumid_registry_key_path(app_id: &str) -> String {
    format!("{AUMID_REGISTRY_KEY_PREFIX}\\{app_id}")
}

/// ヌル終端の UTF-16 文字列へ変換する
fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `AppUserModelID` 用レジストリキーのパスを組み立てること
    #[test]
    fn aumid_registry_key_path_includes_app_id() {
        assert_eq!(
            aumid_registry_key_path("com.example.app"),
            r"Software\Classes\AppUserModelId\com.example.app"
        );
    }

    /// 埋め込み ICO が空でないこと
    #[test]
    fn registry_icon_bytes_not_empty() {
        assert!(!REGISTRY_ICON_BYTES.is_empty());
    }

    /// `\\?\` プレフィックスを除去できること
    #[test]
    fn strip_verbatim_prefix_removes_extended_path_prefix() {
        let path = Path::new(r"\\?\C:\Users\test\notification-icon.ico");
        assert_eq!(
            strip_verbatim_prefix(path),
            r"C:\Users\test\notification-icon.ico"
        );
    }

    /// `REG_EXPAND_SZ` の型が正しいこと
    #[test]
    fn to_reg_expand_sz_uses_expand_sz_type() {
        assert_eq!(to_reg_expand_sz("ClipRefiner").vtype, REG_EXPAND_SZ);
    }
}
