//! Rust と `WebView` UI の間で受け渡す JSON が `ui/ipc.d.ts` の型宣言と一致することのテスト
//!
//! Rust の実際の型から JSON を作り、TypeScript のソースへ埋め込んで `tsc` で型検査する
//! Rust → JS は `window.*` の呼び出し引数として、JS → Rust は `satisfies` と Rust 側のパースの両方で検証する
//! (Node.js と `npm ci` 済みの TypeScript が必要。無い場合の扱いは `run_typescript_check` に従う)

use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use super::apply::SettingsApplyFeedback;
use super::ipc::{SettingsIpcAction, parse_settings_ipc_message};
use super::model::SettingsWritable;
use super::regex_preview::RegexPreviewResponse;
use super::snapshot::SettingsSnapshot;
use super::window::{clip_edit_result_payload, clip_editor_payload};
use crate::config::{AppConfig, MonitorMode, RegexSettings, SelectorWebviewRetention};
use crate::platform::update::{UpdateApplyUiResponse, UpdateCheckUiResponse};
use crate::refiner::RefineMode;
use crate::test_helpers::run_typescript_check;
use crate::tray::selector::{
    clip_selector_focus_script, clip_selector_json, clip_selector_refresh_script,
    quick_selector_focus_script, quick_selector_json, quick_selector_refresh_script,
};
use crate::tray::state::RegexPreviewRequest;

// ======================================================================
// サンプル (Rust → JS)
// ======================================================================

/// 登録クリップとお気に入りを含む設定 (空の一覧では要素の型を検証できないため)
fn sample_config() -> AppConfig {
    let mut config = AppConfig {
        favorite_modes: vec![RefineMode::Trim, RefineMode::JsonFormat],
        ..AppConfig::default()
    };
    config
        .add_registered_clip("hello {counter}")
        .expect("登録クリップの追加");
    config.set_registered_clip_template(0, true);
    config.set_registered_clip_counter(0, 3);
    config
}

/// 値を TypeScript の式 (JSON リテラル) にする
fn ts_literal(value: &impl Serialize) -> String {
    serde_json::to_string_pretty(value).expect("サンプルの JSON 化")
}

/// 文字列 JSON を TypeScript の式にする
fn ts_literal_str(json: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(json).expect("サンプル JSON のパース");
    ts_literal(&value)
}

/// 更新確認の応答 (全バリアント)
fn update_check_samples() -> Vec<UpdateCheckUiResponse> {
    let samples = vec![
        UpdateCheckUiResponse::UpToDate {
            current_version: "1.0.0".to_string(),
            latest_version: "1.0.0".to_string(),
        },
        UpdateCheckUiResponse::Available {
            current_version: "1.0.0".to_string(),
            latest_version: "1.1.0".to_string(),
            tag: "v1.1.0".to_string(),
            asset_name: "ClipRefiner.msi".to_string(),
            size: 1024,
        },
        UpdateCheckUiResponse::Error {
            message: "失敗".to_string(),
        },
        UpdateCheckUiResponse::Busy,
    ];
    // バリアントを足したらサンプルにも足す (網羅漏れをコンパイルエラーにする)
    for sample in &samples {
        match sample {
            UpdateCheckUiResponse::UpToDate { .. }
            | UpdateCheckUiResponse::Available { .. }
            | UpdateCheckUiResponse::Error { .. }
            | UpdateCheckUiResponse::Busy => {}
        }
    }
    samples
}

/// 更新適用の応答 (全バリアント)
fn update_apply_samples() -> Vec<UpdateApplyUiResponse> {
    let samples = vec![
        UpdateApplyUiResponse::MsiStarted {
            message: "起動".to_string(),
        },
        UpdateApplyUiResponse::PortableExtracted {
            path: "C:/ClipRefiner".to_string(),
            message: "展開".to_string(),
        },
        UpdateApplyUiResponse::Error {
            message: "失敗".to_string(),
        },
        UpdateApplyUiResponse::Busy {
            message: "進行中".to_string(),
        },
    ];
    for sample in &samples {
        match sample {
            UpdateApplyUiResponse::MsiStarted { .. }
            | UpdateApplyUiResponse::PortableExtracted { .. }
            | UpdateApplyUiResponse::Error { .. }
            | UpdateApplyUiResponse::Busy { .. } => {}
        }
    }
    samples
}

/// 監視方式 (全バリアント)
fn monitor_mode_samples() -> Vec<MonitorMode> {
    let samples = vec![MonitorMode::Polling, MonitorMode::Event];
    for sample in &samples {
        match sample {
            MonitorMode::Polling | MonitorMode::Event => {}
        }
    }
    samples
}

/// セレクター `WebView` の保持方針 (全バリアント)
fn selector_webview_retention_samples() -> Vec<SelectorWebviewRetention> {
    let samples = vec![
        SelectorWebviewRetention::Keep,
        SelectorWebviewRetention::DisposeOnHide,
        SelectorWebviewRetention::DisposeAfterIdle,
    ];
    for sample in &samples {
        match sample {
            SelectorWebviewRetention::Keep
            | SelectorWebviewRetention::DisposeOnHide
            | SelectorWebviewRetention::DisposeAfterIdle => {}
        }
    }
    samples
}

// ======================================================================
// サンプル (JS → Rust)
// ======================================================================

/// 設定ウィンドウが送る IPC メッセージ (全種類)
fn settings_message_samples() -> Vec<serde_json::Value> {
    let full_patch = SettingsWritable::from_config(&sample_config());
    let clip = |message_type: &str| serde_json::json!({ "type": message_type, "index": 0, "token": "0000000000000001" });
    vec![
        serde_json::json!({ "type": "close" }),
        serde_json::json!({ "type": "open-config" }),
        serde_json::json!({ "type": "open-document", "target": "user-docs" }),
        serde_json::json!({ "type": "open-document", "target": "license" }),
        serde_json::json!({ "type": "open-document", "target": "third-party-notices" }),
        serde_json::json!({ "type": "apply", "patch": full_patch }),
        serde_json::json!({
            "type": "apply",
            "patch": {
                "historyEnabled": true,
                "notificationSettings": { "enabled": true },
                "hotkeys": { "quickSelector": "Alt+Shift+Q" },
            },
        }),
        serde_json::json!({ "type": "clear-history" }),
        serde_json::json!({ "type": "clear-record" }),
        clip("clip-copy"),
        clip("clip-delete"),
        serde_json::json!({
            "type": "clip-move", "index": 0, "token": "0000000000000001", "direction": "up",
        }),
        serde_json::json!({
            "type": "clip-move", "index": 0, "token": "0000000000000001", "direction": "down",
        }),
        serde_json::json!({
            "type": "clip-template", "index": 0, "token": "0000000000000001", "enabled": true,
        }),
        clip("clip-counter-reset"),
        clip("clip-edit-open"),
        serde_json::json!({
            "type": "clip-edit",
            "index": 0,
            "token": "0000000000000001",
            "label": "ラベル",
            "text": "本文",
        }),
        serde_json::json!({ "type": "clip-register" }),
        serde_json::json!({ "type": "favorite-toggle", "mode": "Trim" }),
        serde_json::json!({ "type": "favorite-move", "mode": "Trim", "direction": "up" }),
        serde_json::json!({
            "type": "regex-preview",
            "requestId": 1,
            "settings": RegexSettings::single("a", "b"),
            "sample": "abc",
        }),
        serde_json::json!({ "type": "check-update" }),
        serde_json::json!({ "type": "apply-update" }),
    ]
}

/// IPC メッセージの種類の数 (`settings_message_variant_index` の範囲)
const SETTINGS_MESSAGE_VARIANTS: usize = 19;

/// IPC メッセージの種類を番号にする
///
/// バリアントを足したら番号と `SETTINGS_MESSAGE_VARIANTS` を増やし、サンプルにも足す
fn settings_message_variant_index(action: &SettingsIpcAction) -> usize {
    match action {
        SettingsIpcAction::Close => 0,
        SettingsIpcAction::OpenConfigFile => 1,
        SettingsIpcAction::OpenDocument(_) => 2,
        SettingsIpcAction::Apply(_) => 3,
        SettingsIpcAction::ClearHistory => 4,
        SettingsIpcAction::ClearContinuousRecord => 5,
        SettingsIpcAction::ClipCopy(_) => 6,
        SettingsIpcAction::ClipDelete(_) => 7,
        SettingsIpcAction::ClipMove(..) => 8,
        SettingsIpcAction::ClipTemplate(..) => 9,
        SettingsIpcAction::ClipCounterReset(_) => 10,
        SettingsIpcAction::ClipEditOpen(_) => 11,
        SettingsIpcAction::ClipEdit(..) => 12,
        SettingsIpcAction::ClipRegister => 13,
        SettingsIpcAction::FavoriteToggle(_) => 14,
        SettingsIpcAction::FavoriteMove(..) => 15,
        SettingsIpcAction::RegexPreview(..) => 16,
        SettingsIpcAction::CheckUpdate => 17,
        SettingsIpcAction::ApplyUpdate => 18,
    }
}

// ======================================================================
// TypeScript の生成
// ======================================================================

/// `ui/` ディレクトリの絶対パス (`/` 区切り。TypeScript の import に使う)
fn ui_dir() -> String {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let dir = dir.canonicalize().unwrap_or(dir);
    dir.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
}

/// Rust が `WebView` へ渡す値で `window.*` を呼ぶ TypeScript の文を組み立てる
fn rust_to_js_calls(config: &AppConfig) -> Vec<String> {
    let snapshot = SettingsSnapshot::from_config(config);
    let clips_json = SettingsSnapshot::clips_json(config);
    let modes_json = quick_selector_json(config);
    let selector_clips_json = clip_selector_json(config);

    let regex_ok = RegexPreviewResponse::from_request(&RegexPreviewRequest {
        request_id: 1,
        settings: RegexSettings::single("l+", "L"),
        sample: "hello".to_string(),
    });
    let regex_invalid = RegexPreviewResponse::from_request(&RegexPreviewRequest {
        request_id: 2,
        settings: RegexSettings::single("(", ""),
        sample: "hello".to_string(),
    });

    let mut calls = Vec::new();
    // 設定ウィンドウ (`json_parse_call_script` / `settings_load_script` が渡す値)
    calls.push(format!(
        "window.loadSettings({}, {{ force: true, externalSync: true }});",
        ts_literal_str(&snapshot.to_json())
    ));
    calls.push(format!(
        "window.updateClips({});",
        ts_literal_str(&clips_json)
    ));
    calls.push(format!(
        "window.updateFavoritesAndShortcuts({});",
        ts_literal_str(&SettingsSnapshot::favorites_and_shortcuts_json(config))
    ));
    calls.push(format!(
        "window.showClipEditor({});",
        ts_literal(&clip_editor_payload(0, 1, "ラベル", "本文"))
    ));
    for ok in [true, false] {
        calls.push(format!(
            "window.onClipEditResult({});",
            ts_literal(&clip_edit_result_payload(ok, "理由"))
        ));
    }
    for feedback in [
        SettingsApplyFeedback::ok("保存しました"),
        SettingsApplyFeedback::error(Some("regex"), "不正な正規表現"),
        SettingsApplyFeedback::error(None, "失敗"),
    ] {
        calls.push(format!(
            "window.onApplyFeedback({});",
            ts_literal(&feedback)
        ));
    }
    for response in [&regex_ok, &regex_invalid] {
        calls.push(format!("window.onRegexPreview({});", ts_literal(response)));
    }
    for response in update_check_samples() {
        calls.push(format!("window.onUpdateCheck({});", ts_literal(&response)));
    }
    for response in update_apply_samples() {
        calls.push(format!("window.onUpdateApply({});", ts_literal(&response)));
    }
    // セレクター (Rust が生成するスクリプトは JSON をそのまま埋め込むため、そのまま型検査できる)
    calls.push(format!(
        "window.{}",
        quick_selector_focus_script(Some(RefineMode::Trim), &modes_json, &["URL操作".into()])
    ));
    calls.push(format!(
        "window.{}",
        quick_selector_focus_script(None, &modes_json, &[])
    ));
    calls.push(format!(
        "window.{}",
        quick_selector_refresh_script(Some(RefineMode::Trim), &modes_json)
    ));
    calls.push(format!(
        "window.{}",
        clip_selector_focus_script(&selector_clips_json)
    ));
    calls.push(format!(
        "window.{}",
        clip_selector_refresh_script(&selector_clips_json)
    ));
    calls.push("window.showClipFullText(0, 'hello');".to_string());

    calls
}

/// 検証用の TypeScript ソースを組み立てる
fn build_contract_source() -> String {
    let config = sample_config();
    let calls = rust_to_js_calls(&config);
    let monitor_modes = ts_literal(&monitor_mode_samples());
    let retentions = ts_literal(&selector_webview_retention_samples());
    let messages = ts_literal(&settings_message_samples());
    let check_statuses = ts_literal(&update_check_samples());
    let apply_statuses = ts_literal(&update_apply_samples());
    let ui = ui_dir();

    format!(
        r"// webview_contract_tests.rs が生成する検証用ソース
import type {{
    MonitorMode,
    SelectorWebviewRetention,
    SettingsIpcMessage,
    UpdateApplyResponse,
    UpdateCheckResponse,
}} from '{ui}/ipc';
import type {{ QuickSelectorMode }} from '{ui}/quick_selector_logic';

/** `T` が `never` (網羅済み) であること */
type AssertNever<T extends never> = T;

// ======================================================================
// Rust → JS
// ======================================================================
{calls}

// `focusInput` は 2 つのセレクターで引数が異なり型を付けられないため、一覧だけ別に検査する
export const quickModes = {modes} satisfies QuickSelectorMode[];

const monitorModes = {monitor_modes} satisfies MonitorMode[];
export type MonitorModesCovered = AssertNever<Exclude<MonitorMode, (typeof monitorModes)[number]>>;

const retentions = {retentions} satisfies SelectorWebviewRetention[];
export type RetentionsCovered = AssertNever<
    Exclude<SelectorWebviewRetention, (typeof retentions)[number]>
>;

const checkResponses = {check_statuses} satisfies UpdateCheckResponse[];
export type CheckCovered = AssertNever<
    Exclude<UpdateCheckResponse['status'], (typeof checkResponses)[number]['status']>
>;

const applyResponses = {apply_statuses} satisfies UpdateApplyResponse[];
export type ApplyCovered = AssertNever<
    Exclude<UpdateApplyResponse['status'], (typeof applyResponses)[number]['status']>
>;

// ======================================================================
// JS → Rust
// ======================================================================
const messages = {messages} satisfies SettingsIpcMessage[];
export type MessagesCovered = AssertNever<
    Exclude<SettingsIpcMessage['type'], (typeof messages)[number]['type']>
>;
",
        calls = calls.join("\n"),
        modes = ts_literal_str(&quick_selector_json(&config)),
    )
}

/// 生成したソースを一時ディレクトリに置き、`ui/tsconfig.json` の設定で型検査する
///
/// 失敗したときは調べられるように一時ディレクトリを残し、場所を出力する
fn typecheck_contract_source(source: &str) {
    let dir = std::env::temp_dir().join(format!(
        "clip-refiner-webview-contract-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("一時ディレクトリの作成");
    let tsconfig = serde_json::json!({
        "extends": format!("{}/tsconfig.json", ui_dir()),
        "include": ["contract.ts"],
    });
    fs::write(dir.join("contract.ts"), source).expect("検証用ソースの書き込み");
    fs::write(
        dir.join("tsconfig.json"),
        serde_json::to_string_pretty(&tsconfig).expect("tsconfig の JSON 化"),
    )
    .expect("tsconfig の書き込み");

    let project = dir.join("tsconfig.json");
    if let Err(panic) = std::panic::catch_unwind(|| run_typescript_check(&project)) {
        eprintln!(
            "note: 検証用ソースを {} に残した。Rust の型に合わせて `ui/ipc.d.ts` を直す",
            dir.join("contract.ts").display()
        );
        std::panic::resume_unwind(panic);
    }
    let _ = fs::remove_dir_all(&dir);
}

// ======================================================================
// テスト
// ======================================================================

/// Rust が渡す JSON と設定ウィンドウが送るメッセージが `ui/ipc.d.ts` の型を満たすこと
#[test]
fn webview_payloads_match_ipc_type_declarations() {
    typecheck_contract_source(&build_contract_source());
}

/// TypeScript 側の型検査に使うメッセージを Rust がすべて受け付け、全種類を網羅すること
#[test]
fn settings_message_samples_parse_and_cover_all_variants() {
    let mut seen = [false; SETTINGS_MESSAGE_VARIANTS];
    for sample in settings_message_samples() {
        let json = sample.to_string();
        let action = parse_settings_ipc_message(&json)
            .unwrap_or_else(|| panic!("Rust が解釈できない IPC メッセージ: {json}"));
        seen[settings_message_variant_index(&action)] = true;
    }
    let missing: Vec<usize> = seen
        .iter()
        .enumerate()
        .filter_map(|(index, seen)| (!seen).then_some(index))
        .collect();
    assert!(
        missing.is_empty(),
        "サンプルの無い IPC メッセージ (settings_message_variant_index の番号): {missing:?}"
    );
}
