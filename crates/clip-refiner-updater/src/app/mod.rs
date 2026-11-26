//! 更新プログラム (`ClipRefinerUpdater.exe`) の本体
//!
//! 画面 (`dialog`) を主スレッドで表示し、更新の進行 (`flow`) を別スレッドで実行する
//! 画面を表示できない場合は、アプリを終了する前に中止する

mod args;
mod dialog;
mod flow;
mod logging;
mod ops;
mod page;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use clip_refiner_core::consts::APP_ID;

use super::current_app_version;
use super::error::UpdateError;
use super::named_event::NamedEvent;
use super::replace::{
    canonicalize_dir, is_strict_descendant, schedule_dir_delete, show_error_box, update_work_root,
};
use args::Args;
use dialog::DialogUi;
use flow::{Ui, run_flow};
use ops::SystemOps;
use page::Page;

// ======================================================================
// エントリポイント
// ======================================================================
/// 更新プログラムを実行する
///
/// # Returns
/// * `ExitCode` - 画面を表示できた場合は成功 (更新の成否は画面で知らせる)
pub fn run_updater() -> ExitCode {
    let Some(args) = Args::parse(std::env::args_os().skip(1)) else {
        show_error_box("設定ウィンドウの「更新を確認」から起動してください");
        return ExitCode::FAILURE;
    };

    let code = run_with_args(&args);
    schedule_session_cleanup();
    code
}

/// 多重起動を避けて更新画面を表示する
fn run_with_args(args: &Args) -> ExitCode {
    // 保持している間だけ存在する名前で、起動中の更新プログラムを判定する
    let instance = match NamedEvent::create(&format!(r"Local\{APP_ID}.updater"), true) {
        Ok(instance) if instance.already_exists() => {
            dialog::activate_existing();
            return ExitCode::SUCCESS;
        }
        Ok(instance) => instance,
        Err(_) => {
            show_error_box(&UpdateError::UpdaterLaunch.to_string());
            return ExitCode::FAILURE;
        }
    };

    logging::init(&args.log_dir);
    tracing::info!(
        "更新プログラムを起動しました: version={} install_dir={}",
        current_app_version(),
        args.install_dir.display()
    );
    let code = run_dialog(args);
    drop(instance);
    code
}

/// 更新画面を表示し、閉じられるまで待つ
fn run_dialog(args: &Args) -> ExitCode {
    let ops = match SystemOps::new(args) {
        Ok(ops) => ops,
        Err(err) => {
            show_error_box(&err.to_string());
            return ExitCode::FAILURE;
        }
    };

    let shown = dialog::run(&Page::Checking, &args.log_dir, move |ui| {
        run_guarded(ui, ops);
    });
    match shown {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::warn!("更新画面を表示できない: {err}");
            show_error_box("更新画面を表示できないため、更新を中止しました");
            ExitCode::FAILURE
        }
    }
}

/// 更新を進める。内部エラーで止まっても結果を画面に残す
fn run_guarded(ui: &DialogUi, mut ops: SystemOps) {
    let finished = catch_unwind(AssertUnwindSafe(|| run_flow(ui, &mut ops)));
    if finished.is_err() {
        tracing::error!("更新処理が異常終了した");
        ui.ask(Page::InternalError {
            reason: UpdateError::Internal.to_string(),
        });
    }
}

/// `%TEMP%` へコピーされた自身のフォルダの削除を予約する
fn schedule_session_cleanup() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let (Some(Ok(dir)), Ok(root)) = (
        exe.parent().map(canonicalize_dir),
        canonicalize_dir(&update_work_root()),
    ) else {
        return;
    };
    if is_strict_descendant(&dir, &root) {
        schedule_dir_delete(&dir);
    }
}
