//! 更新プログラムの引数

use std::ffi::OsString;
use std::path::PathBuf;

use crate::session::{ARG_INSTALL_DIR, ARG_LOG_DIR, ARG_WAIT_PID};

// ======================================================================
// 引数
// ======================================================================
/// 常駐アプリから渡される引数 (すべて必須)
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Args {
    /// `ClipRefiner.exe` のあるフォルダ
    pub install_dir: PathBuf,
    /// 終了を待つ常駐アプリの PID
    pub wait_pid: u32,
    /// ログの出力先フォルダ
    pub log_dir: PathBuf,
}

impl Args {
    /// 引数を解釈する
    ///
    /// # Arguments
    /// * `args` - 実行ファイル名を除いた引数
    ///
    /// # Returns
    /// * `Option<Self>` - 不足・重複・未知の引数がある場合は `None`
    pub(super) fn parse(args: impl IntoIterator<Item = OsString>) -> Option<Self> {
        let mut install_dir = None;
        let mut wait_pid = None;
        let mut log_dir = None;

        let mut args = args.into_iter();
        while let Some(name) = args.next() {
            let value = args.next()?;
            let replaced = match name.to_str()? {
                ARG_INSTALL_DIR => install_dir.replace(PathBuf::from(value)).is_some(),
                ARG_LOG_DIR => log_dir.replace(PathBuf::from(value)).is_some(),
                ARG_WAIT_PID => wait_pid.replace(value.to_str()?.parse().ok()?).is_some(),
                _ => return None,
            };
            if replaced {
                return None;
            }
        }

        Some(Self {
            install_dir: install_dir.filter(|dir| dir.is_absolute())?,
            wait_pid: wait_pid?,
            log_dir: log_dir.filter(|dir| dir.is_absolute())?,
        })
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::session::updater_args;

    fn parse(args: &[&str]) -> Option<Args> {
        Args::parse(args.iter().map(OsString::from))
    }

    /// 常駐アプリが渡す引数を解釈できること
    #[test]
    fn parses_launcher_args() {
        let args = updater_args(Path::new(r"C:\My Apps\bin"), 42, Path::new(r"C:\logs"));
        assert_eq!(
            Args::parse(args),
            Some(Args {
                install_dir: PathBuf::from(r"C:\My Apps\bin"),
                wait_pid: 42,
                log_dir: PathBuf::from(r"C:\logs"),
            })
        );
    }

    /// 不足・重複・未知・不正な値の引数を拒否すること
    #[test]
    fn rejects_invalid_args() {
        let valid = [
            "--install-dir",
            r"C:\app",
            "--wait-pid",
            "1",
            "--log-dir",
            r"C:\logs",
        ];
        assert!(parse(&valid).is_some());
        assert!(parse(&[]).is_none());
        assert!(parse(&valid[..4]).is_none());
        assert!(parse(&valid[..5]).is_none());
        assert!(parse(&[&valid[..], &["--wait-pid", "2"]].concat()).is_none());
        assert!(parse(&[&valid[..], &["--other", "x"]].concat()).is_none());

        for (index, value) in [(1, "relative"), (3, "abc"), (5, "relative")] {
            let mut args = valid;
            args[index] = value;
            assert!(parse(&args).is_none(), "{args:?}");
        }
    }
}
