//! Microsoft Edge `WebView2` Runtime の検出と導入案内

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

// ======================================================================
// 定数
// ======================================================================
/// Evergreen `WebView2` Runtime のクライアント GUID
const WEBVIEW2_CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// `WebView2` Runtime のダウンロード案内ページ
pub const DOWNLOAD_PAGE_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

/// プロセス内でダウンロードページを開いたか
static DOWNLOAD_PAGE_OPENED: AtomicBool = AtomicBool::new(false);

/// Runtime 未導入を示唆するエラー文言
const MISSING_RUNTIME_ERROR_HINTS: &[&str] = &[
    "couldn't find a compatible webview2",
    "could not find a compatible webview2",
    "webview2 runtime not found",
    "no installed webview2",
    "webview2runtime not found",
];

#[cfg(test)]
thread_local! {
    /// テスト用の Runtime 有無上書き (`None` なら実レジストリを参照)
    static RUNTIME_AVAILABLE_OVERRIDE: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
    /// テスト用: 既定ブラウザ起動を抑止し URL を記録する
    static OPEN_URL_CAPTURE_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// キャプチャした URL
    static CAPTURED_OPEN_URL: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

// ======================================================================
// パブリック関数
// ======================================================================
/// `WebView2` Runtime が利用可能かどうかを返す
///
/// レジストリの `pv` が存在し、空でも `0.0.0.0` でもない場合に `true`
///
/// # Returns
/// * `bool` - Runtime が導入済みと判定できた場合は `true`
#[must_use]
pub fn is_runtime_available() -> bool {
    #[cfg(test)]
    if let Some(override_value) = RUNTIME_AVAILABLE_OVERRIDE.with(std::cell::Cell::get) {
        return override_value;
    }

    runtime_registry_paths().any(|(hive, path)| {
        read_runtime_pv(&hive, &path).is_some_and(|pv| is_valid_runtime_version(&pv))
    })
}

/// エラー内容が Runtime 未導入を示唆するかどうかを返す
///
/// # Arguments
/// * `err` - `WebView` 初期化失敗の原因
///
/// # Returns
/// * `bool` - 未導入・非互換 Runtime を疑う場合は `true`
#[must_use]
pub fn error_suggests_missing_runtime(err: &anyhow::Error) -> bool {
    let message = format!("{err:#}").to_ascii_lowercase();
    MISSING_RUNTIME_ERROR_HINTS
        .iter()
        .any(|hint| message.contains(hint))
        || (message.contains("webview2") && message.contains("not found"))
}

/// Runtime 未導入時の案内が必要かどうかを返す
///
/// レジストリ上未導入、またはエラーが未導入を示唆する場合に `true`
///
/// # Arguments
/// * `err` - `WebView` 初期化失敗の原因
///
/// # Returns
/// * `bool` - 導入案内を出すべき場合は `true`
#[must_use]
pub fn should_guide_runtime_install(err: &anyhow::Error) -> bool {
    !is_runtime_available() || error_suggests_missing_runtime(err)
}

/// ダウンロード案内ページを既定ブラウザで開く (プロセス内で1回まで)
///
/// # Returns
/// * `Result<bool>` - 実際に起動した場合は `Ok(true)`、既に開済みなら `Ok(false)`
pub fn open_download_page_once() -> Result<bool> {
    if DOWNLOAD_PAGE_OPENED.swap(true, Ordering::SeqCst) {
        return Ok(false);
    }
    open_url(DOWNLOAD_PAGE_URL)?;
    Ok(true)
}

/// 起動時に Runtime 未導入ならデスクトップ通知で案内する
///
/// ブラウザは開かない (ユーザー操作なしの起動を避ける)
pub fn notify_missing_runtime_on_startup() {
    if is_runtime_available() {
        return;
    }
    crate::user_error::webview2_runtime_missing_startup().notify();
}

/// テスト用に Runtime 有無を上書きする (`None` で解除)
///
/// # Arguments
/// * `value` - 上書きする有無。`None` なら実レジストリ参照に戻す
#[cfg(test)]
pub fn set_runtime_available_for_test(value: Option<bool>) {
    RUNTIME_AVAILABLE_OVERRIDE.with(|cell| cell.set(value));
}

/// テスト用に URL 起動キャプチャを開始する
#[cfg(test)]
pub fn begin_capture_open_url_for_test() {
    CAPTURED_OPEN_URL.with(|cell| *cell.borrow_mut() = None);
    OPEN_URL_CAPTURE_ENABLED.with(|cell| cell.set(true));
    DOWNLOAD_PAGE_OPENED.store(false, Ordering::SeqCst);
}

/// キャプチャした URL を取り出し、キャプチャを終了する
///
/// # Returns
/// * `Option<String>` - 開こうとした URL。未起動なら `None`
#[cfg(test)]
pub fn take_captured_open_url_for_test() -> Option<String> {
    OPEN_URL_CAPTURE_ENABLED.with(|cell| cell.set(false));
    CAPTURED_OPEN_URL.with(|cell| cell.borrow_mut().take())
}

/// テスト用に「ダウンロードページを開いた」フラグをリセットする
#[cfg(test)]
pub fn reset_download_page_opened_for_test() {
    DOWNLOAD_PAGE_OPENED.store(false, Ordering::SeqCst);
}

// ======================================================================
// プライベート関数
// ======================================================================
/// Runtime バージョンを探すレジストリパス一覧
fn runtime_registry_paths() -> impl Iterator<Item = (RegKey, String)> {
    [
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_GUID}"),
        ),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_GUID}"),
        ),
        (
            RegKey::predef(HKEY_CURRENT_USER),
            format!(r"Software\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_GUID}"),
        ),
        (
            RegKey::predef(HKEY_CURRENT_USER),
            format!(r"Software\Microsoft\EdgeUpdate\ClientState\{WEBVIEW2_CLIENT_GUID}"),
        ),
    ]
    .into_iter()
}

/// 指定キーから `pv` を読む
fn read_runtime_pv(hive: &RegKey, path: &str) -> Option<String> {
    let key = hive.open_subkey(path).ok()?;
    key.get_value::<String, _>("pv").ok()
}

/// Runtime バージョン文字列が導入済みとして有効か
fn is_valid_runtime_version(pv: &str) -> bool {
    let trimmed = pv.trim();
    !trimmed.is_empty() && trimmed != "0.0.0.0"
}

/// URL を既定ブラウザで開く
fn open_url(url: &str) -> Result<()> {
    #[cfg(test)]
    if OPEN_URL_CAPTURE_ENABLED.with(std::cell::Cell::get) {
        CAPTURED_OPEN_URL.with(|cell| *cell.borrow_mut() = Some(url.to_string()));
        return Ok(());
    }

    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .with_context(|| format!("URL のオープンに失敗: {url}"))?;
    Ok(())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 有効な `pv` のみ導入済みと判定すること
    #[test]
    fn valid_runtime_version_rejects_empty_and_zero() {
        assert!(is_valid_runtime_version("120.0.2210.91"));
        assert!(!is_valid_runtime_version(""));
        assert!(!is_valid_runtime_version("   "));
        assert!(!is_valid_runtime_version("0.0.0.0"));
    }

    /// `WebView2` 未導入を示唆するエラー文言を検出すること
    #[test]
    fn error_suggests_missing_runtime_detects_common_messages() {
        let err = anyhow::anyhow!("Couldn't find a compatible WebView2 Runtime installation");
        assert!(error_suggests_missing_runtime(&err));

        let other = anyhow::anyhow!("WebView2 データディレクトリの作成に失敗");
        assert!(!error_suggests_missing_runtime(&other));
    }

    /// レジストリ未導入なら案内対象とすること
    #[test]
    fn should_guide_when_runtime_unavailable() {
        set_runtime_available_for_test(Some(false));
        let err = anyhow::anyhow!("WebView の構築に失敗");
        assert!(should_guide_runtime_install(&err));
        set_runtime_available_for_test(None);
    }

    /// Runtime ありでも未導入エラーなら案内対象とすること
    #[test]
    fn should_guide_when_error_suggests_missing_even_if_registry_present() {
        set_runtime_available_for_test(Some(true));
        let err = anyhow::anyhow!("Couldn't find a compatible WebView2 Runtime installation");
        assert!(should_guide_runtime_install(&err));
        set_runtime_available_for_test(None);
    }

    /// ダウンロードページはプロセス内で1回だけ開くようにすること
    #[test]
    fn open_download_page_once_opens_only_first_time() {
        reset_download_page_opened_for_test();
        begin_capture_open_url_for_test();
        assert!(open_download_page_once().expect("1回目"));
        assert_eq!(
            take_captured_open_url_for_test().as_deref(),
            Some(DOWNLOAD_PAGE_URL)
        );

        OPEN_URL_CAPTURE_ENABLED.with(|cell| cell.set(true));
        CAPTURED_OPEN_URL.with(|cell| *cell.borrow_mut() = None);
        assert!(!open_download_page_once().expect("2回目"));
        assert!(take_captured_open_url_for_test().is_none());
        reset_download_page_opened_for_test();
    }

    /// クライアント GUID 定数がレジストリパスに使われているようにすること
    #[test]
    fn registry_paths_include_webview2_guid() {
        let paths: Vec<_> = runtime_registry_paths().map(|(_, path)| path).collect();
        assert!(paths.iter().all(|path| path.contains(WEBVIEW2_CLIENT_GUID)));
        assert!(paths.len() >= 3);
    }

    /// 実環境の Runtime 検出がパニックしないようにすること
    #[test]
    fn is_runtime_available_does_not_panic() {
        set_runtime_available_for_test(None);
        let _ = is_runtime_available();
    }
}
