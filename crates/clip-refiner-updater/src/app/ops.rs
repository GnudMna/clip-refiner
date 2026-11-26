//! 更新の副作用の実装

use std::path::PathBuf;
use std::time::Duration;

use semver::Version;

use super::args::Args;
use super::flow::Ops;
use crate::check::check_for_update;
use crate::error::UpdateError;
use crate::installed::{AutostartSnapshot, installed_version};
use crate::msi::{application_folder, install_msi};
use crate::prepare::{prepare_update, remove_work_dir};
use crate::replace::{install_portable, validate_portable};
use crate::resident::{ResidentProcess, ResidentStarter};
use crate::types::{
    AvailableUpdate, InstallOutcome, PreparedPackage, UpdateCheckOutcome, UpdatePackageKind,
};

// ======================================================================
// 定数
// ======================================================================
/// `msiexec` の詳細ログのファイル名 (更新のたびに上書きする)
const MSI_LOG_FILE_NAME: &str = "updater-msi.log";

// ======================================================================
// 型
// ======================================================================
/// 実際のインストール先に対する副作用
pub(super) struct SystemOps {
    install_dir: PathBuf,
    log_dir: PathBuf,
    kind: UpdatePackageKind,
    resident: ResidentProcess,
    starter: ResidentStarter,
    /// 維持する MSI のインストール先 (`ensure_updatable` で確定する)
    application_folder: Option<PathBuf>,
}

impl SystemOps {
    /// 引数から組み立てる
    ///
    /// # Arguments
    /// * `args` - 更新プログラムの引数
    ///
    /// # Returns
    /// * `Result<Self, UpdateError>` - 終了を待つプロセスが正しくない場合はエラー
    pub(super) fn new(args: &Args) -> Result<Self, UpdateError> {
        Ok(Self {
            kind: UpdatePackageKind::for_install_dir(&args.install_dir),
            resident: ResidentProcess::open(args.wait_pid, &args.install_dir)?,
            starter: ResidentStarter::new(&args.install_dir),
            application_folder: None,
            install_dir: args.install_dir.clone(),
            log_dir: args.log_dir.clone(),
        })
    }
}

impl Ops for SystemOps {
    fn check(&self) -> UpdateCheckOutcome {
        check_for_update(self.kind)
    }

    fn ensure_updatable(&mut self) -> Result<(), UpdateError> {
        if self.kind == UpdatePackageKind::Msi {
            self.application_folder = Some(application_folder(&self.install_dir)?);
        }
        Ok(())
    }

    fn prepare(
        &self,
        update: &AvailableUpdate,
        on_progress: &mut dyn FnMut(u64),
    ) -> Result<PreparedPackage, UpdateError> {
        let package = prepare_update(update, on_progress)?;
        // 差し替えられない状態なら、アプリを終了する前に止める
        if let PreparedPackage::Portable { source_dir } = &package {
            validate_portable(source_dir, &self.install_dir)?;
        }
        Ok(package)
    }

    fn request_exit(&self) -> Result<(), UpdateError> {
        self.resident.request_exit()
    }

    fn wait_exit(&self, timeout: Duration) -> Result<bool, UpdateError> {
        self.resident.wait_exit(timeout)
    }

    fn install(&mut self, package: &PreparedPackage) -> Result<InstallOutcome, UpdateError> {
        match package {
            PreparedPackage::Msi { path } => {
                let folder = self
                    .application_folder
                    .as_deref()
                    .ok_or(UpdateError::InstallLocationMismatch)?;
                let autostart = AutostartSnapshot::capture();
                let result = install_msi(path, folder, &self.log_dir.join(MSI_LOG_FILE_NAME));
                if let Some(autostart) = autostart {
                    autostart.restore();
                }
                result
            }
            PreparedPackage::Portable { source_dir } => {
                install_portable(source_dir, &self.install_dir)?;
                Ok(InstallOutcome::Installed)
            }
        }
    }

    fn installed_version(&self) -> Option<Version> {
        installed_version(&self.install_dir)
    }

    fn start(&mut self) -> Result<(), UpdateError> {
        self.starter.start()
    }

    fn cleanup(&self, update: &AvailableUpdate) {
        remove_work_dir(&update.latest_version);
    }
}
