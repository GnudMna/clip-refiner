//! 起動オーケストレーション (常駐 / ワンショット / ポータブル差し替え)
//!
//! CLI 引数の解釈、ログ・多重起動防止、トレイ常駐またはワンショット実行への分岐を担う

mod args;
mod instance;
mod logging;
mod oneshot;
mod panic_hook;
mod portable;
mod temp_cleanup;

use anyhow::Result;
use clap::Parser;

use crate::log_error;
use crate::platform;
use crate::refiner;
use crate::tray;
use crate::user_error::fatal_startup;
use args::{Args, is_oneshot};
use instance::ensure_single_instance;
use logging::{setup_console, setup_logging};
use oneshot::run_once;
use portable::run_portable_update_helper;
use temp_cleanup::remove_stale_temp_dirs;

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションの起動処理
///
/// # Returns
/// * `Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
pub fn run() -> Result<()> {
    setup_console();

    let args = Args::parse();
    if args.apply_portable_update {
        return run_portable_update_helper(&args);
    }

    if args.e2e_window.is_some() {
        tray::e2e::activate();
    }

    let _guard = match setup_logging() {
        Ok(guard) => guard,
        Err(err) => {
            report_fatal_startup_error("ログの初期化", &err);
            return Err(err);
        }
    };

    platform::init_notifications();

    // 常駐起動のみ多重起動を防止する。ワンショットは常駐中でも実行できる
    let _instance = if is_oneshot(&args) {
        None
    } else {
        Some(match ensure_single_instance(args.e2e_window.is_some()) {
            Ok(instance) => instance,
            Err(err) => {
                report_fatal_startup_error("多重起動防止", &err);
                return Err(err);
            }
        })
    };

    if let Err(err) = run_application(&args) {
        log_error!("アプリケーションの実行に失敗: {:#}", err);
        report_fatal_startup_error("実行", &err);
        return Err(err);
    }

    Ok(())
}

/// 引数に応じて常駐またはワンショット実行を行う
fn run_application(args: &Args) -> Result<()> {
    if is_oneshot(args) {
        run_once(args)
    } else {
        // 前回の起動で書き出した、貼り付け用の画像ファイルのうち古いものを削除する
        refiner::remove_stale_image_files();
        // 更新や WebView2 の作業ディレクトリのうち、古いものを削除する
        remove_stale_temp_dirs();
        tray::run_loop(args.e2e_window)
    }
}

/// 起動失敗をログ・標準エラー・デスクトップ通知へ報告する
///
/// Windows の GUI ビルドではコンソールがないため、通知が主なフィードバックになる
fn report_fatal_startup_error(context: &str, err: &anyhow::Error) {
    let notification = fatal_startup(context, err);
    eprintln!("{}: {}", notification.title(), notification.body());
    platform::show_notification(notification.title(), notification.body());
}
