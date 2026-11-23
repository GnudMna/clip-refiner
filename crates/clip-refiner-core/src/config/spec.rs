//! 設定仕様の同期カタログ
//!
//! 実装の既定値 (`AppConfig` / `consts`)・生成 TOML コメント・configuration.md / README を
//! 突き合わせ、スキーマ番号や上限・既定ホットキーのドリフトを検出する

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "テスト専用モジュールのため、失敗はそのまま panic させる"
)]

use std::fs;
use std::path::{Path, PathBuf};

use super::serialize::config_to_toml;
use super::serialize::docs::{
    DOC_AUTO_REFINE_ENABLED, DOC_MODE, DOC_VERSION, doc_continuous_record_limit,
    doc_favorite_modes, doc_history_limit, doc_hotkey_favorite_slots, doc_interval_ms,
    doc_pipeline, doc_regex_presets, doc_selector_webview_idle_minutes, section_hotkeys,
};
use super::types::{AppConfig, HotkeySettings, NotificationSettings};
use crate::consts;
use crate::security::{format_kib, format_mib};

// ======================================================================
// ワークスペースファイル
// ======================================================================
/// 設定リファレンスのワークスペース相対パス
const CONFIG_DOC: &str = "docs/user/configuration.md";

/// ワークスペースルート (リポジトリ直下) を返す
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("ワークスペースルートの解決")
}

/// ワークスペース内のテキストファイルを読む
fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} を読めない ({}): {err}", path.display(), relative))
}

/// Markdown の指定見出しから、同じレベル以上の次の見出しまでを返す
///
/// `###` は `## ` を部分文字列に含むため、単純な `## ` 検索だと下位見出しで切れてしまう
fn markdown_section<'a>(md: &'a str, heading: &str) -> &'a str {
    let start = md
        .find(heading)
        .unwrap_or_else(|| panic!("見出しが見つからない: {heading}"));
    let level = heading.chars().take_while(|&ch| ch == '#').count();
    let rest = &md[start + heading.len()..];
    let mut search_from = 0;
    while let Some(rel) = rest[search_from..].find('\n') {
        let newline_at = search_from + rel;
        let line = &rest[newline_at + 1..];
        let hashes = line.chars().take_while(|&ch| ch == '#').count();
        if hashes > 0 && hashes <= level && line[hashes..].starts_with(' ') {
            return &rest[..newline_at];
        }
        search_from = newline_at + 1;
    }
    rest
}

/// 設定項目表からキーに対応するデフォルト列を返す
fn config_table_default(section: &str, key: &str) -> String {
    let needle = format!("`{key}`");
    for line in section.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line
            .split('|')
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .collect();
        let Some(key_cell) = cells.first() else {
            continue;
        };
        if *key_cell != needle.as_str() {
            continue;
        }
        return cells
            .get(2)
            .unwrap_or_else(|| panic!("デフォルト列がない: {key}"))
            .trim_matches('`')
            .to_string();
    }
    panic!("configuration.md の設定項目表に `{key}` がない")
}

/// 空配列以外の TOML 文字列既定値を返す
fn quoted(value: &str) -> String {
    format!("\"{value}\"")
}

/// 列挙値の TOML 既定値 (`"UrlDecode"` 形式) を返す
fn quoted_debug(value: &impl std::fmt::Debug) -> String {
    format!("\"{value:?}\"")
}

// ======================================================================
// カタログ
// ======================================================================
/// 設定項目表で同期するキーと、実装から得た既定値表示
struct SpecField {
    key: &'static str,
    default_cell: String,
}

/// `AppConfig::default` と `consts` から設定項目表の期待値を組み立てる
fn spec_fields(config: &AppConfig) -> Vec<SpecField> {
    let mut fields = spec_root_fields(config);
    fields.extend(spec_notification_fields(&config.notification_settings));
    fields.extend(spec_hotkey_fields(&config.hotkeys));
    fields.push(SpecField {
        key: "regex.active_preset_id",
        default_cell: quoted(&config.regex.active_preset_id),
    });
    fields
}

/// ルートキーの設定項目表期待値を返す
fn spec_root_fields(config: &AppConfig) -> Vec<SpecField> {
    vec![
        SpecField {
            key: "version",
            default_cell: config.version.to_string(),
        },
        SpecField {
            key: "auto_refine_enabled",
            default_cell: config.auto_refine_enabled.to_string(),
        },
        SpecField {
            key: "mode",
            default_cell: quoted_debug(&config.mode),
        },
        SpecField {
            key: "pipeline",
            default_cell: empty_array_cell(&config.pipeline),
        },
        SpecField {
            key: "favorite_modes",
            default_cell: empty_array_cell(&config.favorite_modes),
        },
        SpecField {
            key: "interval_ms",
            default_cell: config.interval_ms.to_string(),
        },
        SpecField {
            key: "monitor_mode",
            default_cell: quoted_debug(&config.monitor_mode),
        },
        SpecField {
            key: "selector_webview_retention",
            default_cell: quoted_debug(&config.selector_webview_retention),
        },
        SpecField {
            key: "selector_webview_idle_minutes",
            default_cell: config.selector_webview_idle_minutes.to_string(),
        },
        SpecField {
            key: "history_enabled",
            default_cell: config.history_enabled.to_string(),
        },
        SpecField {
            key: "continuous_record_enabled",
            default_cell: config.continuous_record_enabled.to_string(),
        },
        SpecField {
            key: "continuous_record_limit",
            default_cell: config.continuous_record_limit.to_string(),
        },
        SpecField {
            key: "history_limit",
            default_cell: config.history_limit.to_string(),
        },
    ]
}

/// 通知キーの設定項目表期待値を返す
fn spec_notification_fields(ns: &NotificationSettings) -> Vec<SpecField> {
    vec![
        SpecField {
            key: "notification_settings.enabled",
            default_cell: ns.enabled.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_process",
            default_cell: ns.notify_process.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_auto_refine",
            default_cell: ns.notify_auto_refine.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_history_restore",
            default_cell: ns.notify_history_restore.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_ocr",
            default_cell: ns.notify_ocr.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_registered_clip",
            default_cell: ns.notify_registered_clip.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_favorites",
            default_cell: ns.notify_favorites.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_continuous_record",
            default_cell: ns.notify_continuous_record.to_string(),
        },
        SpecField {
            key: "notification_settings.notify_result",
            default_cell: ns.notify_result.to_string(),
        },
    ]
}

/// ホットキーの設定項目表期待値を返す
fn spec_hotkey_fields(hk: &HotkeySettings) -> Vec<SpecField> {
    vec![
        SpecField {
            key: "hotkeys.quick_selector",
            default_cell: quoted(&hk.quick_selector),
        },
        SpecField {
            key: "hotkeys.clip_selector",
            default_cell: quoted(&hk.clip_selector),
        },
        SpecField {
            key: "hotkeys.ocr",
            default_cell: quoted(&hk.ocr),
        },
        SpecField {
            key: "hotkeys.notification",
            default_cell: quoted(&hk.notification),
        },
        SpecField {
            key: "hotkeys.auto_refine",
            default_cell: quoted(&hk.auto_refine),
        },
        SpecField {
            key: "hotkeys.continuous_record",
            default_cell: quoted(&hk.continuous_record),
        },
        SpecField {
            key: "hotkeys.continuous_record_undo",
            default_cell: quoted(&hk.continuous_record_undo),
        },
        SpecField {
            key: "hotkeys.continuous_record_paste",
            default_cell: quoted(&hk.continuous_record_paste),
        },
        SpecField {
            key: "hotkeys.undo",
            default_cell: quoted(&hk.undo),
        },
        SpecField {
            key: "hotkeys.quit",
            default_cell: quoted(&hk.quit),
        },
        SpecField {
            key: "hotkeys.favorite_mode_slots",
            default_cell: empty_array_cell(&hk.favorite_mode_slots),
        },
    ]
}

/// 空配列を configuration.md の `(空)` 表記に合わせる
fn empty_array_cell<T>(items: &[T]) -> String {
    if items.is_empty() {
        "(空)".to_string()
    } else {
        panic!("空配列以外の既定値は設定項目表の特例表記がない")
    }
}

/// 処理の制限表に含めるべき実装由来の断片
fn processing_limit_needles() -> Vec<String> {
    vec![
        format_mib(consts::MAX_CLIPBOARD_TEXT_BYTES),
        format_mib(consts::MAX_PARSER_INPUT_BYTES),
        format!("{} 階層", consts::MAX_HTML_NESTING_DEPTH),
        format_kib(consts::MAX_REGEX_PATTERN_BYTES),
        format!("{} 件", consts::MAX_REGEX_PRESETS),
        format!("{} 件", consts::MAX_REGEX_STEPS_PER_PRESET),
        format!("{} 秒", consts::REGEX_MATCH_TIMEOUT_MS / 1000),
        format!("{} 本", consts::REGEX_MATCH_MAX_IN_FLIGHT),
        format!("{} 件", consts::MAX_REGISTERED_CLIPS),
        format!("{} 文字", consts::MAX_REGISTERED_CLIP_LABEL_CHARS),
        format_mib(consts::MAX_REGISTERED_IMAGE_BYTES),
        format!("{} px", consts::MAX_REGISTERED_IMAGE_DIMENSION),
        format_mib(consts::MAX_REGISTERED_CLIPS_FILE_BYTES),
        format!("{} px", consts::MAX_BASE64_IMAGE_DIMENSION),
        format!("{} ピクセル", consts::MAX_BASE64_IMAGE_PIXELS),
        format!(
            "{} 時間",
            consts::CLIPBOARD_IMAGE_FILE_RETENTION_SECS / 3600
        ),
        format!("{} 件", consts::MAX_FAVORITE_MODES),
        format!("{} 段", consts::MAX_PIPELINE_LENGTH),
        format!(
            "{}〜{} ms",
            consts::MIN_INTERVAL_MS,
            consts::MAX_INTERVAL_MS
        ),
    ]
}

/// 既定ホットキー (README / configuration.md の本文にも現れる)
fn default_hotkeys() -> [&'static str; 10] {
    [
        consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
        consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
        consts::DEFAULT_HOTKEY_OCR,
        consts::DEFAULT_HOTKEY_NOTIFICATION,
        consts::DEFAULT_HOTKEY_AUTO_REFINE,
        consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
        consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_UNDO,
        consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_PASTE,
        consts::DEFAULT_HOTKEY_UNDO,
        consts::DEFAULT_HOTKEY_QUIT,
    ]
}

/// 生成 TOML に含める説明コメント
fn generated_toml_comments() -> Vec<String> {
    vec![
        DOC_VERSION.to_string(),
        DOC_AUTO_REFINE_ENABLED.to_string(),
        DOC_MODE.to_string(),
        doc_pipeline(),
        doc_favorite_modes(),
        doc_interval_ms(),
        doc_selector_webview_idle_minutes(),
        doc_continuous_record_limit(),
        doc_history_limit(),
        doc_regex_presets(),
        section_hotkeys(),
    ]
}

// ======================================================================
// テスト
// ======================================================================
/// `###` 下位見出しで設定項目節が切れないこと
#[test]
fn markdown_section_keeps_h3_inside_h2() {
    let md = "\
## 設定項目

intro

### 基本

| `version` | number | `3` | x |

## 変換モード一覧

rest
";
    let section = markdown_section(md, "## 設定項目");
    assert!(
        section.contains("### 基本"),
        "h3 が節に残ること:\n{section}"
    );
    assert!(
        section.contains("`version`"),
        "h3 配下の表が節に残ること:\n{section}"
    );
    assert!(
        !section.contains("変換モード一覧"),
        "次の h2 は含めないこと:\n{section}"
    );
}

/// configuration.md のスキーマ番号が `CONFIG_VERSION` と一致すること
#[test]
fn config_md_schema_version_matches_consts() {
    let md = read_workspace_file(CONFIG_DOC);
    let version = consts::CONFIG_VERSION;
    assert!(
        md.contains(&format!("現行の設定スキーマは `version = {version}`")),
        "configuration.md 冒頭のスキーマ番号が {version} ではない"
    );
    assert!(
        md.contains("移行手順が無いスキーマ"),
        "configuration.md に未対応 version の既存項目維持方針が無い"
    );

    let items = markdown_section(&md, "## 設定項目");
    assert_eq!(
        config_table_default(items, "version"),
        version.to_string(),
        "設定項目表の version 既定値が {version} ではない"
    );

    for example in toml_code_fences(&md) {
        if !example.contains("version =") {
            continue;
        }
        assert!(
            example.contains(&format!("version = {version}")),
            "configuration.md の TOML 例の version が {version} ではない:\n{example}"
        );
    }
}

/// 設定項目表の既定値が `AppConfig::default` と一致すること
#[test]
fn config_md_item_defaults_match_app_config() {
    let md = read_workspace_file(CONFIG_DOC);
    let items = markdown_section(&md, "## 設定項目");
    let config = AppConfig::default();
    assert_eq!(config.version, consts::CONFIG_VERSION);

    for field in spec_fields(&config) {
        let actual = config_table_default(items, field.key);
        assert_eq!(
            actual, field.default_cell,
            "configuration.md の `{}` 既定値が実装と異なる",
            field.key
        );
    }
}

/// 処理の制限が `consts` の上限と一致すること
#[test]
fn config_md_processing_limits_match_consts() {
    let md = read_workspace_file(CONFIG_DOC);
    let limits = markdown_section(&md, "## 処理の制限");
    for needle in processing_limit_needles() {
        assert!(
            limits.contains(&needle),
            "configuration.md の処理の制限に `{needle}` がない"
        );
    }
}

/// README / configuration.md に既定ホットキーが含まれること
#[test]
fn readme_and_config_list_default_hotkeys() {
    let readme = read_workspace_file("README.md");
    let config_md = read_workspace_file(CONFIG_DOC);
    for hotkey in default_hotkeys() {
        assert!(
            readme.contains(hotkey),
            "README.md に既定ホットキー `{hotkey}` がない"
        );
        assert!(
            config_md.contains(hotkey),
            "configuration.md に既定ホットキー `{hotkey}` がない"
        );
    }
}

/// 生成 TOML の schema・コメントが実装と一致すること
#[test]
fn generated_toml_matches_spec_comments_and_version() {
    let config = AppConfig::default();
    let toml = config_to_toml(&config, None).expect("コメント付き TOML の生成");
    assert!(
        toml.contains(&format!("version = {}", consts::CONFIG_VERSION)),
        "生成 TOML の version が CONFIG_VERSION ではない:\n{toml}"
    );
    for comment in generated_toml_comments() {
        assert!(
            toml.contains(&format!("# {comment}")),
            "生成 TOML にコメント `{comment}` がない"
        );
    }
    for hotkey in default_hotkeys() {
        assert!(
            toml.contains(&format!("= \"{hotkey}\"")),
            "生成 TOML に既定ホットキー `{hotkey}` がない"
        );
    }

    let mut with_slots = AppConfig::default();
    with_slots.hotkeys.favorite_mode_slots =
        vec![consts::DEFAULT_HOTKEY_QUICK_SELECTOR.to_string()];
    let toml_with_slots =
        config_to_toml(&with_slots, None).expect("お気に入りスロット付き TOML の生成");
    let favorite_comment = doc_hotkey_favorite_slots();
    assert!(
        toml_with_slots.contains(&format!("# {favorite_comment}")),
        "生成 TOML にコメント `{favorite_comment}` がない"
    );
}

/// Markdown 内の `toml` コードフェンスを列挙する
fn toml_code_fences(md: &str) -> Vec<&str> {
    let mut fences = Vec::new();
    let mut rest = md;
    while let Some(start) = rest.find("```toml") {
        let after = &rest[start + "```toml".len()..];
        let Some(end) = after.find("```") else {
            break;
        };
        fences.push(after[..end].trim());
        rest = &after[end + 3..];
    }
    fences
}
