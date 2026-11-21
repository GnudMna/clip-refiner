//! `WebView` E2E (Playwright + CDP) 用の起動ヘルパー
//!
//! `--e2e-window` と `CLIP_REFINER_E2E_CDP_PORT` で有効化する

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::consts;

use clap::ValueEnum;

// ======================================================================
// 定数
// ======================================================================
/// 準備完了時に stdout へ出すマーカー行 (Playwright が待機する)
pub const E2E_READY_MARKER: &str = "E2E_READY";

/// `wry` 既定相当の `WebView2` 引数 (追加指定時に自分で付与する必要がある)
const WEBVIEW2_DEFAULT_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

// ======================================================================
// 状態
// ======================================================================
static E2E_ACTIVE: AtomicBool = AtomicBool::new(false);

// ======================================================================
// 対象ウィンドウ
// ======================================================================
/// E2E 起動時に自動表示する `WebView` ウィンドウ
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum E2eWindow {
    /// 設定ウィンドウ
    Settings,
    /// クイックセレクター
    Quick,
    /// 登録クリップセレクター
    Clip,
}

// ======================================================================
// パブリック関数
// ======================================================================
/// E2E モードを有効化する
pub fn activate() {
    E2E_ACTIVE.store(true, Ordering::SeqCst);
}

/// E2E モードが有効かどうか
///
/// # Returns
/// * `bool` - `--e2e-window` 起動時は `true`
pub fn is_active() -> bool {
    E2E_ACTIVE.load(Ordering::SeqCst)
}

/// CDP ポート番号を環境変数から読む
///
/// # Returns
/// * `Option<u16>` - 有効なポートが指定されていればその値
pub fn cdp_port() -> Option<u16> {
    let raw = std::env::var(consts::ENV_E2E_CDP_PORT).ok()?;
    let port: u16 = raw.parse().ok()?;
    (port > 0).then_some(port)
}

/// `WebView2` に渡す追加ブラウザ引数 (CDP 有効時のみ)
///
/// # Returns
/// * `Option<String>` - CDP ポート指定時はデフォルト引数 + remote-debugging-port
pub fn additional_browser_args() -> Option<String> {
    let port = cdp_port()?;
    Some(format!(
        "{WEBVIEW2_DEFAULT_BROWSER_ARGS} --remote-debugging-port={port}"
    ))
}

/// E2E 用に `WebView2` データディレクトリ接尾辞へポートを付与する
///
/// 追加ブラウザ引数が異なるインスタンスはデータディレクトリを分ける必要がある
///
/// # Arguments
/// * `base_suffix` - 通常の接尾辞
///
/// # Returns
/// * `String` - E2E 時は接尾辞にポートを付与した値、それ以外は `base_suffix`
pub fn webview_data_dir_suffix(base_suffix: &str) -> String {
    match cdp_port() {
        Some(port) => format!("{base_suffix}-e2e-{port}"),
        None => base_suffix.to_string(),
    }
}

/// 対象 `WebView` の準備完了を stdout へ通知する
pub fn signal_ready() {
    if !is_active() {
        return;
    }
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "{E2E_READY_MARKER}");
    let _ = stdout.flush();
}
