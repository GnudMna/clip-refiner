//! 更新の進行 (確認 → ダウンロード → 終了待ち → インストール → 起動確認 → 結果)
//!
//! 画面と副作用はトレイトで受け取り、成否の判定をここに集める

use std::time::{Duration, Instant};

use semver::Version;

use super::page::{Action, AppStatus, Page};
use crate::error::UpdateError;
use crate::installed::matches_version;
use crate::types::{AvailableUpdate, InstallOutcome, PreparedPackage, UpdateCheckOutcome};

// ======================================================================
// 定数
// ======================================================================
/// アプリの終了を待つ上限
const EXIT_TIMEOUT: Duration = Duration::from_mins(2);
/// 終了待ちの間に中止の要求を調べる間隔
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// 中止後、既に受け付けられた終了の依頼が済むのを待つ上限
const CANCEL_GRACE: Duration = Duration::from_secs(5);

// ======================================================================
// 画面と副作用
// ======================================================================
/// 更新画面
pub(super) trait Ui {
    /// ページを表示する (操作を待たない)
    fn show(&self, page: Page);
    /// ページを表示し、利用者の操作を待つ
    fn ask(&self, page: Page) -> Action;
    /// ダウンロードの進捗を表示する
    fn progress(&self, received: u64, total: u64);
    /// 打ち切れない段階へ入る。既に打ち切られていれば `false`
    fn begin_apply(&self) -> bool;
    /// 利用者が中止を求めたか
    fn cancel_requested(&self) -> bool;
}

/// 更新の副作用
pub(super) trait Ops {
    /// 最新リリースを確認する
    fn check(&self) -> UpdateCheckOutcome;
    /// このインストール先を更新できるか確かめる
    fn ensure_updatable(&mut self) -> Result<(), UpdateError>;
    /// ダウンロード・検証・展開する
    fn prepare(
        &self,
        update: &AvailableUpdate,
        on_progress: &mut dyn FnMut(u64),
    ) -> Result<PreparedPackage, UpdateError>;
    /// アプリへ終了を依頼する
    fn request_exit(&self) -> Result<(), UpdateError>;
    /// アプリが動作していない状態になるのを待つ。動作していなければ `true`
    fn wait_exit(&self, timeout: Duration) -> Result<bool, UpdateError>;
    /// インストールする
    fn install(&mut self, package: &PreparedPackage) -> Result<InstallOutcome, UpdateError>;
    /// インストールされている版を返す
    fn installed_version(&self) -> Option<Version>;
    /// アプリを起動し、常駐の完了を待つ
    fn start(&mut self) -> Result<(), UpdateError>;
    /// ダウンロードしたファイルを片付ける
    fn cleanup(&self, update: &AvailableUpdate);
}

// ======================================================================
// 進行
// ======================================================================
/// 更新を最後まで進め、利用者が結果を閉じたら戻る
///
/// # Arguments
/// * `ui` - 更新画面
/// * `ops` - 更新の副作用
pub(super) fn run_flow(ui: &impl Ui, ops: &mut impl Ops) {
    ui.show(Page::Checking);
    let update = match ops.check() {
        UpdateCheckOutcome::Available(update) => update,
        UpdateCheckOutcome::UpToDate {
            current_version, ..
        } => {
            ui.ask(Page::UpToDate {
                current: current_version,
            });
            return;
        }
        UpdateCheckOutcome::Error { message } => {
            ui.ask(Page::CheckFailed { reason: message });
            return;
        }
    };

    let choice = ui.ask(Page::Available {
        current: update.current_version.clone(),
        latest: update.latest_version.clone(),
        size: update.size,
    });
    if choice != Action::Update {
        return;
    }

    let package = match prepare(ui, ops, &update) {
        Ok(package) => package,
        Err(reason) => {
            ops.cleanup(&update);
            // アプリにはまだ触れていない。利用者が終了していた場合も起動しない
            tracing::warn!("更新の準備に失敗: {reason}");
            let running = ops.wait_exit(Duration::ZERO) != Ok(true);
            let status = app_status(ops, running);
            finish(ui, ops, status, |status| Page::Failed {
                reason: reason.to_string(),
                status,
            });
            return;
        }
    };
    if ui.begin_apply() {
        apply(ui, ops, &update, &package);
    }
    ops.cleanup(&update);
}

/// アプリを動かしたまま、インストールの準備を済ませる
fn prepare(
    ui: &impl Ui,
    ops: &mut impl Ops,
    update: &AvailableUpdate,
) -> Result<PreparedPackage, UpdateError> {
    ops.ensure_updatable()?;
    ui.show(Page::Downloading {
        latest: update.latest_version.clone(),
        size: update.size,
    });
    ops.prepare(update, &mut |received| ui.progress(received, update.size))
}

/// アプリを終了してインストールし、新しい版の動作を確かめる
fn apply(ui: &impl Ui, ops: &mut impl Ops, update: &AvailableUpdate, package: &PreparedPackage) {
    ui.show(Page::WaitingExit);
    match wait_for_exit(ui, ops) {
        Ok(true) => {}
        Ok(false) => {
            // 依頼を受け付けた後のアプリは、中止しても終了する
            let _ = ops.wait_exit(CANCEL_GRACE);
            let status = ensure_running(ui, ops);
            finish(ui, ops, status, |status| Page::Canceled { status });
            return;
        }
        Err(reason) => {
            finish_failed(ui, ops, &reason);
            return;
        }
    }

    ui.show(Page::Installing {
        latest: update.latest_version.clone(),
    });
    let installed = ops.install(package);
    let reached = ops
        .installed_version()
        .is_some_and(|version| matches_version(&version, &update.latest_version));
    match installed {
        Ok(InstallOutcome::Installed) if reached => {
            start_new_version(ui, ops, &update.latest_version);
        }
        Ok(InstallOutcome::Installed) => {
            finish_failed(ui, ops, &UpdateError::InstalledVersionMismatch);
        }
        Ok(InstallOutcome::RebootRequired) => {
            let status = ensure_running(ui, ops);
            finish(ui, ops, status, |status| Page::RebootRequired {
                latest: update.latest_version.clone(),
                status,
            });
        }
        Err(reason) => finish_failed(ui, ops, &reason),
    }
}

/// アプリの終了を依頼して待つ。中止された場合は `Ok(false)`
fn wait_for_exit(ui: &impl Ui, ops: &impl Ops) -> Result<bool, UpdateError> {
    ops.request_exit()?;
    let deadline = Instant::now() + EXIT_TIMEOUT;
    while !ops.wait_exit(EXIT_POLL_INTERVAL)? {
        if ui.cancel_requested() {
            return Ok(false);
        }
        if Instant::now() >= deadline {
            return Err(UpdateError::ExitTimeout);
        }
    }
    // 終了と同時に中止された場合も、中止を優先する
    Ok(!ui.cancel_requested())
}

/// 新しい版を起動し、動作を確認できるまで結果を表示する
fn start_new_version(ui: &impl Ui, ops: &mut impl Ops, latest: &str) {
    loop {
        ui.show(Page::Starting);
        match ops.start() {
            Ok(()) => {
                tracing::info!("{latest} への更新が完了しました");
                ui.ask(Page::Succeeded {
                    latest: latest.to_string(),
                });
                return;
            }
            Err(reason) => {
                tracing::warn!("{latest} の動作を確認できない: {reason}");
                let choice = ui.ask(Page::StartFailed {
                    latest: latest.to_string(),
                    reason: reason.to_string(),
                });
                if choice != Action::Start {
                    return;
                }
            }
        }
    }
}

// ======================================================================
// 失敗時の後始末
// ======================================================================
/// 終了しているアプリを起動し直し、失敗の結果を表示する
fn finish_failed(ui: &impl Ui, ops: &mut impl Ops, reason: &UpdateError) {
    tracing::warn!("更新に失敗: {reason}");
    let status = ensure_running(ui, ops);
    finish(ui, ops, status, |status| Page::Failed {
        reason: reason.to_string(),
        status,
    });
}

/// 結果を表示する。停止中のアプリの起動を選ばれたら、起動してから表示し直す
fn finish(
    ui: &impl Ui,
    ops: &mut impl Ops,
    mut status: AppStatus,
    page: impl Fn(AppStatus) -> Page,
) {
    while ui.ask(page(status)) == Action::Start {
        status = start_app(ui, ops);
    }
}

/// アプリが終了していれば起動し、状態を返す
fn ensure_running(ui: &impl Ui, ops: &mut impl Ops) -> AppStatus {
    if ops.wait_exit(Duration::ZERO) == Ok(true) {
        return start_app(ui, ops);
    }
    app_status(ops, true)
}

/// アプリを起動し、状態を返す
fn start_app(ui: &impl Ui, ops: &mut impl Ops) -> AppStatus {
    ui.show(Page::Starting);
    let started = ops.start();
    if let Err(err) = &started {
        tracing::warn!("ClipRefiner を起動し直せない: {err}");
    }
    app_status(ops, started.is_ok())
}

/// インストールされている版を添えた状態を返す
fn app_status(ops: &impl Ops, running: bool) -> AppStatus {
    AppStatus {
        version: ops.installed_version().map(|version| version.to_string()),
        running,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::path::PathBuf;

    use super::*;

    /// 表示したページを記録し、決めた順に操作を返す画面
    struct FakeUi {
        pages: RefCell<Vec<Page>>,
        actions: RefCell<VecDeque<Action>>,
        aborted: bool,
        cancel: bool,
        progress: Cell<u64>,
    }

    impl FakeUi {
        /// 更新を選び、結果のページでは `results` の順に操作する
        fn updating(results: &[Action]) -> Self {
            let mut actions = VecDeque::from([Action::Update]);
            actions.extend(results);
            Self {
                pages: RefCell::default(),
                actions: RefCell::new(actions),
                aborted: false,
                cancel: false,
                progress: Cell::new(0),
            }
        }

        fn last_page(&self) -> Page {
            self.pages.borrow().last().cloned().expect("page")
        }
    }

    impl Ui for FakeUi {
        fn show(&self, page: Page) {
            self.pages.borrow_mut().push(page);
        }

        fn ask(&self, page: Page) -> Action {
            self.show(page);
            self.actions
                .borrow_mut()
                .pop_front()
                .unwrap_or(Action::Close)
        }

        fn progress(&self, received: u64, _total: u64) {
            self.progress.set(received);
        }

        fn begin_apply(&self) -> bool {
            !self.aborted
        }

        fn cancel_requested(&self) -> bool {
            self.cancel
        }
    }

    /// 結果を差し替えられる副作用。呼ばれた順を記録する
    struct FakeOps {
        check: UpdateCheckOutcome,
        updatable: Result<(), UpdateError>,
        prepared: Result<PreparedPackage, UpdateError>,
        request: Result<(), UpdateError>,
        /// 終了の依頼に応じるか
        exits: bool,
        exited: Cell<bool>,
        install: Result<InstallOutcome, UpdateError>,
        version: &'static str,
        version_after_install: &'static str,
        starts: VecDeque<Result<(), UpdateError>>,
        calls: RefCell<Vec<&'static str>>,
    }

    impl FakeOps {
        /// すべて成功する
        fn succeeding() -> Self {
            Self {
                check: UpdateCheckOutcome::Available(AvailableUpdate {
                    current_version: "4.0.1".to_string(),
                    latest_version: "4.1.0".to_string(),
                    tag: "v4.1.0".to_string(),
                    asset_name: "clip-refiner-4.1.0-x86_64.msi".to_string(),
                    size: 100,
                    sha256: "a".repeat(64),
                }),
                updatable: Ok(()),
                prepared: Ok(PreparedPackage::Msi {
                    path: PathBuf::from("app.msi"),
                }),
                request: Ok(()),
                exits: true,
                exited: Cell::new(false),
                install: Ok(InstallOutcome::Installed),
                version: "4.0.1",
                version_after_install: "4.1.0",
                starts: VecDeque::new(),
                calls: RefCell::default(),
            }
        }

        fn called(&self, name: &'static str) -> bool {
            self.calls.borrow().contains(&name)
        }
    }

    impl Ops for FakeOps {
        fn check(&self) -> UpdateCheckOutcome {
            self.check.clone()
        }

        fn ensure_updatable(&mut self) -> Result<(), UpdateError> {
            self.updatable.clone()
        }

        fn prepare(
            &self,
            _update: &AvailableUpdate,
            on_progress: &mut dyn FnMut(u64),
        ) -> Result<PreparedPackage, UpdateError> {
            self.calls.borrow_mut().push("prepare");
            on_progress(100);
            self.prepared.clone()
        }

        fn request_exit(&self) -> Result<(), UpdateError> {
            self.calls.borrow_mut().push("request_exit");
            self.request.clone()?;
            self.exited.set(self.exits);
            Ok(())
        }

        fn wait_exit(&self, _timeout: Duration) -> Result<bool, UpdateError> {
            Ok(self.exited.get())
        }

        fn install(&mut self, _package: &PreparedPackage) -> Result<InstallOutcome, UpdateError> {
            self.calls.borrow_mut().push("install");
            self.version = self.version_after_install;
            self.install.clone()
        }

        fn installed_version(&self) -> Option<Version> {
            Version::parse(self.version).ok()
        }

        fn start(&mut self) -> Result<(), UpdateError> {
            self.calls.borrow_mut().push("start");
            self.starts.pop_front().unwrap_or(Ok(()))
        }

        fn cleanup(&self, _update: &AvailableUpdate) {
            self.calls.borrow_mut().push("cleanup");
        }
    }

    /// 動作中の版の状態
    fn running(version: &str) -> AppStatus {
        AppStatus {
            version: Some(version.to_string()),
            running: true,
        }
    }

    /// 新しい版の起動まで確認できたときだけ成功を表示すること
    #[test]
    fn success_requires_started_new_version() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        run_flow(&ui, &mut ops);

        assert_eq!(
            ui.last_page(),
            Page::Succeeded {
                latest: "4.1.0".to_string()
            }
        );
        assert_eq!(ui.progress.get(), 100);
        assert_eq!(
            *ops.calls.borrow(),
            ["prepare", "request_exit", "install", "start", "cleanup"]
        );
    }

    /// 最新版では何もせずに知らせること
    #[test]
    fn up_to_date_changes_nothing() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.check = UpdateCheckOutcome::UpToDate {
            current_version: "4.1.0".to_string(),
            latest_version: "4.1.0".to_string(),
        };
        run_flow(&ui, &mut ops);

        assert_eq!(
            ui.last_page(),
            Page::UpToDate {
                current: "4.1.0".to_string()
            }
        );
        assert!(ops.calls.borrow().is_empty());
    }

    /// 更新を選ばなければダウンロードしないこと
    #[test]
    fn closing_available_page_does_not_download() {
        let ui = FakeUi::updating(&[]);
        ui.actions.borrow_mut()[0] = Action::Close;
        let mut ops = FakeOps::succeeding();
        run_flow(&ui, &mut ops);

        assert!(ops.calls.borrow().is_empty());
    }

    /// 更新できないインストール先ではダウンロードせず、アプリを終了しないこと
    #[test]
    fn unsupported_location_keeps_app_running() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.updatable = Err(UpdateError::InstallLocationMismatch);
        run_flow(&ui, &mut ops);

        assert!(!ops.called("prepare"));
        assert!(!ops.called("request_exit"));
    }

    /// ダウンロードや検証の失敗では、アプリを終了せずに知らせること
    #[test]
    fn prepare_failure_keeps_app_running() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.prepared = Err(UpdateError::SignatureInvalid);
        run_flow(&ui, &mut ops);

        assert_eq!(
            ui.last_page(),
            Page::Failed {
                reason: UpdateError::SignatureInvalid.to_string(),
                status: running("4.0.1"),
            }
        );
        assert_eq!(*ops.calls.borrow(), ["prepare", "cleanup"]);
    }

    /// 準備に失敗したとき、利用者が終了していたアプリを起動しないこと
    #[test]
    fn prepare_failure_does_not_start_closed_app() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.prepared = Err(UpdateError::DownloadFailed);
        ops.exited.set(true);
        run_flow(&ui, &mut ops);

        assert!(!ops.called("start"));
        assert_eq!(
            ui.last_page(),
            Page::Failed {
                reason: UpdateError::DownloadFailed.to_string(),
                status: AppStatus {
                    running: false,
                    ..running("4.0.1")
                },
            }
        );
    }

    /// 終了と同時に中止されたときは、インストールせずに元の版を起動すること
    #[test]
    fn cancel_at_exit_restarts_old_version() {
        let ui = FakeUi {
            cancel: true,
            ..FakeUi::updating(&[])
        };
        let mut ops = FakeOps::succeeding();
        run_flow(&ui, &mut ops);

        assert!(!ops.called("install"));
        assert!(ops.called("start"));
        assert_eq!(
            ui.last_page(),
            Page::Canceled {
                status: running("4.0.1")
            }
        );
    }

    /// 準備中に打ち切られたら、アプリを終了しないこと
    #[test]
    fn abort_before_apply_keeps_app_running() {
        let ui = FakeUi {
            aborted: true,
            ..FakeUi::updating(&[])
        };
        let mut ops = FakeOps::succeeding();
        run_flow(&ui, &mut ops);

        assert!(!ops.called("request_exit"));
    }

    /// 終了の依頼を届けられないときは、インストールしないこと
    #[test]
    fn exit_request_failure_does_not_install() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.request = Err(UpdateError::ExitRequest);
        run_flow(&ui, &mut ops);

        assert!(!ops.called("install"));
        assert!(!ops.called("start"));
        assert_eq!(
            ui.last_page(),
            Page::Failed {
                reason: UpdateError::ExitRequest.to_string(),
                status: running("4.0.1"),
            }
        );
    }

    /// 終了待ちの中止ではインストールせず、現在の版を伝えること
    #[test]
    fn cancel_while_waiting_does_not_install() {
        let ui = FakeUi {
            cancel: true,
            ..FakeUi::updating(&[])
        };
        let mut ops = FakeOps::succeeding();
        ops.exits = false;
        run_flow(&ui, &mut ops);

        assert!(!ops.called("install"));
        assert_eq!(
            ui.last_page(),
            Page::Canceled {
                status: running("4.0.1")
            }
        );
    }

    /// インストールに失敗したら旧版を起動し、実際の版を伝えること
    #[test]
    fn install_failure_restarts_old_version() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.install = Err(UpdateError::MsiFailed { code: 1603 });
        ops.version_after_install = "4.0.1";
        run_flow(&ui, &mut ops);

        assert!(ops.called("start"));
        assert_eq!(
            ui.last_page(),
            Page::Failed {
                reason: UpdateError::MsiFailed { code: 1603 }.to_string(),
                status: running("4.0.1"),
            }
        );
    }

    /// インストーラーが成功を返しても、版が変わっていなければ失敗とすること
    #[test]
    fn unchanged_version_is_failure() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.version_after_install = "4.0.1";
        run_flow(&ui, &mut ops);

        assert_eq!(
            ui.last_page(),
            Page::Failed {
                reason: UpdateError::InstalledVersionMismatch.to_string(),
                status: running("4.0.1"),
            }
        );
    }

    /// 新しい版が起動しないときは成功と表示せず、もう一度起動できること
    #[test]
    fn start_failure_can_be_retried() {
        let ui = FakeUi::updating(&[Action::Start]);
        let mut ops = FakeOps::succeeding();
        ops.starts = VecDeque::from([Err(UpdateError::StartExited), Ok(())]);
        run_flow(&ui, &mut ops);

        assert!(ui.pages.borrow().contains(&Page::StartFailed {
            latest: "4.1.0".to_string(),
            reason: UpdateError::StartExited.to_string(),
        }));
        assert_eq!(
            ui.last_page(),
            Page::Succeeded {
                latest: "4.1.0".to_string()
            }
        );
    }

    /// 旧版を起動し直せなかったときは停止中と伝え、起動を選べること
    #[test]
    fn stopped_old_version_can_be_started() {
        let ui = FakeUi::updating(&[Action::Start]);
        let mut ops = FakeOps::succeeding();
        ops.install = Err(UpdateError::MsiBusy);
        ops.version_after_install = "4.0.1";
        ops.starts = VecDeque::from([Err(UpdateError::StartFailed), Ok(())]);
        run_flow(&ui, &mut ops);

        let page = |status| Page::Failed {
            reason: UpdateError::MsiBusy.to_string(),
            status,
        };
        let stopped = AppStatus {
            running: false,
            ..running("4.0.1")
        };
        assert!(ui.pages.borrow().contains(&page(stopped)));
        assert_eq!(ui.last_page(), page(running("4.0.1")));
    }

    /// Windows の再起動が必要なときは、成功と区別して知らせること
    #[test]
    fn reboot_required_is_reported_separately() {
        let ui = FakeUi::updating(&[]);
        let mut ops = FakeOps::succeeding();
        ops.install = Ok(InstallOutcome::RebootRequired);
        ops.version_after_install = "4.0.1";
        run_flow(&ui, &mut ops);

        assert_eq!(
            ui.last_page(),
            Page::RebootRequired {
                latest: "4.1.0".to_string(),
                status: running("4.0.1"),
            }
        );
    }
}
