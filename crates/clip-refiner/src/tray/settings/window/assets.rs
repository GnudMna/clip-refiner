use crate::tray::menu::icon::app_icon_png_data_url;
use crate::tray::selector::apply_webview_csp;

// ======================================================================
// HTML 組み立て
// ======================================================================
const SETTINGS_CSS_IMPORT: &str = r#"@import url("settings.css");"#;
const SETTINGS_JS_TAG: &str = r#"<script src="settings.js"></script>"#;
const SETTINGS_ICON_PLACEHOLDER: &str = "__APP_ICON_SRC__";
/// OS の閉じる操作から呼ぶ JS (未公開時は no-op)
pub(super) const CLOSE_FROM_OS_SCRIPT: &str =
    "typeof window.closeWindow==='function'&&window.closeWindow();";
pub(super) const FLUSH_PENDING_APPLY_SCRIPT: &str =
    "typeof window.flushPendingApply==='function'&&window.flushPendingApply();";
const IPC_LOG_PREVIEW_MAX_LEN: usize = 200;

/// 解釈に失敗した IPC メッセージを警告ログへ出力する
pub(super) fn log_unparsed_settings_ipc(msg: &str) {
    let preview = if msg.len() > IPC_LOG_PREVIEW_MAX_LEN {
        format!("{}...", &msg[..IPC_LOG_PREVIEW_MAX_LEN])
    } else {
        msg.to_string()
    };
    crate::log_warn!("設定 IPC メッセージの解釈に失敗: {}", preview);
}

/// 設定ウィンドウ用 JavaScript バンドルを返す
///
/// ソースは `src/ui/settings/*.js`。`npm run build:ui` (esbuild) で
/// `settings.bundle.js` を生成し、ここへ埋め込む。`cargo build` 単体では Node を要求しない
pub(super) fn assemble_settings_js() -> String {
    include_str!("../../../ui/settings/settings.bundle.js").to_string()
}

/// 設定ウィンドウ HTML 内のアセットをインライン展開する
fn embed_settings_assets(html: &str) -> String {
    let css = include_str!("../../../ui/settings.css");
    let js = assemble_settings_js();

    apply_webview_csp(
        &html
            .replace(SETTINGS_CSS_IMPORT, css)
            .replace(SETTINGS_JS_TAG, &format!("<script>\n{js}\n</script>"))
            .replace(SETTINGS_ICON_PLACEHOLDER, &app_icon_png_data_url()),
    )
}

/// 設定ウィンドウ用 HTML を組み立てる
pub(super) fn assemble_settings_html() -> String {
    embed_settings_assets(include_str!("../../../ui/settings.html"))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::{CLOSE_FROM_OS_SCRIPT, assemble_settings_html, assemble_settings_js};

    /// 設定 UI JavaScript に正規表現プレビューの stale 判定ロジックが含まれるようにすること
    #[test]
    fn assembled_settings_js_includes_regex_preview_stale_guard() {
        let js = assemble_settings_js();
        assert!(js.contains("resolveRegexPreviewResponse"));
        assert!(js.contains("pendingRegexPreviewRequest"));
        assert!(js.contains(r#"action: "stale""#));
    }

    /// 設定 UI JavaScript に責務分割後の DOM モジュールが含まれるようにすること
    #[test]
    fn assembled_settings_js_includes_split_dom_modules() {
        let js = assemble_settings_js();
        assert!(js.contains("グローバルホットキーを編集する"));
        assert!(js.contains("function scheduleApply"));
        assert!(js.contains("function bindConfirmDialog"));
        assert!(js.contains("function handleRegexPreview"));
        assert!(js.contains("function updateMonitorPipelineUi"));
        assert!(js.contains("function panelWrap"));
        assert!(js.contains("onUpdateCheck"));
        assert!(js.contains("onUpdateApply"));
        assert!(js.contains("更新を確認"));
        assert!(js.contains("セレクター WebView"));
        assert!(js.contains("selectorWebviewRetention"));
        assert!(js.contains("DisposeAfterIdle"));
        assert!(js.contains("ctx.render = render"));
        assert!(js.contains("ctx.refreshActivePanel = refreshActivePanel"));
    }

    /// 設定 UI に表示マスキングと同一ユーザー信頼境界の注意が含まれるようにすること
    #[test]
    fn assembled_settings_js_explains_display_only_masking() {
        let js = assemble_settings_js();
        assert!(js.contains(
            "マスキングは通知・メニュー・プレビューの表示置換のみ。クリップボード本体は書き換えない"
        ));
        assert!(js.contains(
            "トレイ履歴のラベルでは機密らしい内容を表示のみマスクする。復元時はクリップボードへ元の本文を載せる"
        ));
        assert!(js.contains(
            "プレビューのマスキングは表示のみ。コピー時はクリップボードへ元の本文を載せる"
        ));
        assert!(js.contains(
            "保護境界は現在の Windows ユーザー単位。同一ユーザーの他プロセスからは設定・IPC・鍵に到達しうる"
        ));
        assert!(js.contains(
            "鍵は現在ユーザーの DPAPI で保護する。同一ユーザーの他プロセスからは設定・鍵に到達しうる"
        ));
    }

    /// 設定 UI JavaScript に共有純粋ロジックが含まれるようにすること
    #[test]
    fn assembled_settings_js_includes_shared_logic() {
        let js = assemble_settings_js();
        assert!(js.contains("SettingsSharedLogic"));
        assert!(js.contains("SettingsPanelsLogic"));
        assert!(js.contains("buildNotificationSettingsPatch"));
        assert!(js.contains("normalizePipeline"));
        assert!(js.contains("shouldFullRenderOnLoad"));
        assert!(js.contains("collectCloseWindowIpcMessages"));
        assert!(js.contains("resolveShortcutsDiffUpdate"));
        assert!(js.contains("shouldCloseSettingsOnEscape"));
        assert!(js.contains("applyPipelineAction"));
        assert!(js.contains("resolveContentClickAction"));
        assert!(js.contains("nextTabIndexForKey"));
        assert!(js.contains("tabSelectionState"));
    }

    /// OS の閉じる操作向けに `window.closeWindow` が公開されるようにすること
    #[test]
    fn assembled_settings_js_exposes_close_window_on_window() {
        let js = assemble_settings_js();
        assert!(js.contains("window.closeWindow = closeWindow"));
        assert!(js.contains("collectCloseWindowIpcMessages"));
        assert_eq!(
            CLOSE_FROM_OS_SCRIPT,
            "typeof window.closeWindow==='function'&&window.closeWindow();"
        );
    }

    /// 組み立て HTML にシェル必須 DOM と全タブ定義が含まれるようにすること
    #[test]
    fn assembled_settings_html_includes_shell_and_tabs() {
        let html = assemble_settings_html();
        for marker in [
            r#"id="app""#,
            r#"id="sidebar""#,
            r#"role="tablist""#,
            r#"aria-orientation="vertical""#,
            r#"id="content""#,
            r#"id="open-config-btn""#,
            r#"id="close-btn""#,
            r#"id="apply-feedback""#,
            r#"id="confirm-dialog""#,
            r#"http-equiv="Content-Security-Policy""#,
        ] {
            assert!(
                html.contains(marker),
                "組み立て HTML に {marker} が含まれること"
            );
        }

        let js = assemble_settings_js();
        for tab_id in [
            "general",
            "favorites",
            "regex",
            "history",
            "clips",
            "monitor",
            "notifications",
            "shortcuts",
        ] {
            // esbuild 出力はダブルクォート
            let marker = format!(r#"id: "{tab_id}""#);
            assert!(
                js.contains(&marker),
                "組み立て JS にタブ {tab_id} が含まれること"
            );
        }
    }

    /// CSS / JS がインライン展開され外部参照が残らないようにすること
    #[test]
    fn assembled_settings_html_inlines_assets() {
        let html = assemble_settings_html();
        assert!(!html.contains(r#"@import url("settings.css")"#));
        assert!(!html.contains(r#"<script src="settings.js"></script>"#));
        assert!(!html.contains("__APP_ICON_SRC__"));
        assert!(html.contains("<script>"));
        assert!(html.contains("SettingsSharedLogic"));
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains(r#"http-equiv="Content-Security-Policy""#));
        assert!(!html.contains("__WEBVIEW_CSP__"));
    }
}
