//! ポータブル更新の差し替えヘルパー

use anyhow::{Result, bail};

use super::args::Args;
use crate::platform;

// ======================================================================
// ポータブル差し替えヘルパー
// ======================================================================
/// ポータブル更新の差し替えヘルパーを実行する
///
/// ログ初期化や多重起動防止を行わず、ファイル差し替えだけを担当する
///
/// # Arguments
/// * `args` - CLI 引数
///
/// # Returns
/// * `Result<()>` - 差し替え成功時 `Ok(())`
pub(super) fn run_portable_update_helper(args: &Args) -> Result<()> {
    let Some(wait_pid) = args.wait_pid else {
        bail!("--apply-portable-update には --wait-pid が必要です");
    };
    let Some(install_dir) = args.install_dir.clone() else {
        bail!("--apply-portable-update には --install-dir が必要です");
    };

    platform::init_notifications();
    if let Err(err) =
        platform::update::run_portable_replace(wait_pid, &install_dir, args.cleanup_dir.as_deref())
    {
        bail!("{err}");
    }
    Ok(())
}
