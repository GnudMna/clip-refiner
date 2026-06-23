//! ログイン時の自動起動を OS ネイティブの仕組みで制御する

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

// ======================================================================
// 公開 API
// ======================================================================
/// ログイン時の自動起動が有効かどうかを返す
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

/// ログイン時の自動起動を有効または無効にする
///
/// # Arguments
/// * `enabled` - 有効にする場合は `true`、無効にする場合は `false`
///
/// # Returns
/// * `Result<()>` - 設定変更に成功した場合は `Ok(())`、失敗した場合は `Err` を返す
pub fn set_enabled(enabled: bool) -> Result<()> {
    platform::set_enabled(enabled)
}

// ======================================================================
// 共通ヘルパー
// ======================================================================
/// 現在の実行ファイルパスを正規化して返す
fn current_exe_path() -> Result<PathBuf> {
    std::env::current_exe()
        .context("実行ファイルパスの取得に失敗")
        .map(normalize_path)
}

/// パスを正規化する (`canonicalize` 失敗時は入力パスをそのまま返す)
fn normalize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// 2 つのパスが同一の実行ファイルを指すかどうかを判定する
fn paths_match(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

// ======================================================================
// プラットフォーム実装
// ======================================================================
#[cfg(windows)]
mod platform {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    use super::{current_exe_path, normalize_path, paths_match};
    use crate::consts;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    /// レジストリの Run キーに登録されているかどうかを返す
    pub fn is_enabled() -> bool {
        let Ok(exe_path) = current_exe_path() else {
            return false;
        };

        match read_run_value() {
            Ok(Some(value)) => paths_match(&value, &exe_path),
            _ => false,
        }
    }

    /// レジストリの Run キーへ登録または削除する
    pub fn set_enabled(enabled: bool) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ)
            .context("自動起動レジストリキーのオープンに失敗")?;

        if enabled {
            let exe_path = current_exe_path()?;
            let path_str = exe_path.to_string_lossy().into_owned();
            run.set_value(consts::APP_NAME, &path_str)
                .context("自動起動レジストリ値の書き込みに失敗")?;
        } else {
            let _ = run.delete_value(consts::APP_NAME);
        }

        Ok(())
    }

    /// Run キーに登録されている実行ファイルパスを読み取る
    fn read_run_value() -> Result<Option<PathBuf>> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .context("自動起動レジストリキーの読み取りに失敗")?;

        match run.get_value::<String, _>(consts::APP_NAME) {
            Ok(value) => Ok(Some(normalize_path(PathBuf::from(value)))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("自動起動レジストリ値の読み取りに失敗"),
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::fs;
    use std::io::Write;

    use anyhow::{Context, Result};

    use super::current_exe_path;
    use crate::consts;

    /// LaunchAgent が配置されているかどうかを返す
    pub fn is_enabled() -> bool {
        launch_agent_path().is_ok_and(|path| path.exists())
    }

    /// LaunchAgent を配置または削除する
    pub fn set_enabled(enabled: bool) -> Result<()> {
        let path = launch_agent_path()?;

        if enabled {
            let exe_path = current_exe_path()?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context("LaunchAgents ディレクトリの作成に失敗")?;
            }

            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
                label = consts::APP_ID,
                exe = xml_escape(&exe_path.to_string_lossy()),
            );

            let mut file = fs::File::create(&path).context("LaunchAgent ファイルの作成に失敗")?;
            file.write_all(plist.as_bytes())
                .context("LaunchAgent ファイルの書き込みに失敗")?;
        } else if path.exists() {
            fs::remove_file(&path).context("LaunchAgent ファイルの削除に失敗")?;
        }

        Ok(())
    }

    /// LaunchAgent の配置先パスを返す
    fn launch_agent_path() -> Result<std::path::PathBuf> {
        let home = directories::BaseDirs::new()
            .context("ホームディレクトリの取得に失敗")?
            .home_dir()
            .to_path_buf();

        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", consts::APP_ID)))
    }

    /// XML 属性値向けに特殊文字をエスケープする
    fn xml_escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::fs;
    use std::io::Write;

    use anyhow::{Context, Result};

    use super::current_exe_path;
    use crate::consts;

    /// XDG autostart の .desktop ファイルが存在するかどうかを返す
    pub fn is_enabled() -> bool {
        desktop_entry_path().is_ok_and(|path| path.exists())
    }

    /// XDG autostart の .desktop ファイルを配置または削除する
    pub fn set_enabled(enabled: bool) -> Result<()> {
        let path = desktop_entry_path()?;

        if enabled {
            let exe_path = current_exe_path()?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context("autostart ディレクトリの作成に失敗")?;
            }

            let desktop = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name={name}\n\
                 Exec={exec}\n\
                 Hidden=false\n\
                 NoDisplay=false\n\
                 X-GNOME-Autostart-enabled=true\n",
                name = consts::APP_NAME,
                exec = exe_path.to_string_lossy(),
            );

            let mut file = fs::File::create(&path).context(".desktop ファイルの作成に失敗")?;
            file.write_all(desktop.as_bytes())
                .context(".desktop ファイルの書き込みに失敗")?;
        } else if path.exists() {
            fs::remove_file(&path).context(".desktop ファイルの削除に失敗")?;
        }

        Ok(())
    }

    /// XDG autostart の .desktop ファイルパスを返す
    fn desktop_entry_path() -> Result<std::path::PathBuf> {
        let config_dir = crate::config::get_config_dir()?;
        Ok(config_dir
            .parent()
            .context("設定ディレクトリの親パス取得に失敗")?
            .join("autostart")
            .join(format!("{}.desktop", consts::APP_NAME_KEBAB)))
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod platform {
    use anyhow::Result;

    /// 未対応プラットフォームでは常に無効
    pub fn is_enabled() -> bool {
        false
    }

    /// 未対応プラットフォームでは何もしない
    pub fn set_enabled(_enabled: bool) -> Result<()> {
        Ok(())
    }
}
