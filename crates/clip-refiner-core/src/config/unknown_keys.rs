// ======================================================================
// 未知設定キーの検出
// ======================================================================
/// ユーザー向けメッセージに含める未知キーの最大件数
const UNKNOWN_KEYS_MESSAGE_LIMIT: usize = 5;

/// ルートレベルの既知キー (レガシー `clips` と v4 以前の `is_paused` を含む)
const ROOT_KEYS: &[&str] = &[
    "version",
    "auto_refine_enabled",
    "mode",
    "pipeline",
    "favorite_modes",
    "interval_ms",
    "monitor_mode",
    "selector_webview_retention",
    "selector_webview_idle_minutes",
    "history_enabled",
    "continuous_record_enabled",
    "continuous_record_limit",
    "history_limit",
    "is_paused",
    "notification_settings",
    "hotkeys",
    "regex",
    "clips",
];

/// `[notification_settings]` の既知キー (serde alias / 旧キーを含む)
const NOTIFICATION_KEYS: &[&str] = &[
    "enabled",
    "notify_process",
    "notifyProcess",
    "notify_auto_refine",
    "notifyAutoRefine",
    "notify_pause",
    "notifyPause",
    "notify_history_restore",
    "notifyHistoryRestore",
    "notify_ocr",
    "notifyOcr",
    "notify_registered_clip",
    "notifyRegisteredClip",
    "notify_favorites",
    "notifyFavorites",
    "notify_continuous_record",
    "notifyContinuousRecord",
    "notify_result",
    "notifyResult",
    "notify_mode",
    "notifyMode",
    "notify_actions",
    "notifyActions",
];

/// `[hotkeys]` の既知キー (serde alias / 旧キーを含む)
const HOTKEY_KEYS: &[&str] = &[
    "quick_selector",
    "quickSelector",
    "selector",
    "notification",
    "auto_refine",
    "autoRefine",
    "pause",
    "quit",
    "undo",
    "clip_selector",
    "clipSelector",
    "text_selector",
    "ocr",
    "continuous_record",
    "continuousRecord",
    "continuous_record_undo",
    "continuousRecordUndo",
    "continuous_record_paste",
    "continuousRecordPaste",
    "paste_queue",
    "favorite_mode_slots",
    "favoriteModeSlots",
];

/// `[regex]` の既知キー (旧フラット形式と serde alias を含む)
const REGEX_KEYS: &[&str] = &[
    "active_preset_id",
    "activePresetId",
    "presets",
    "pattern",
    "replacement",
    "case_insensitive",
    "caseInsensitive",
    "multiline",
];

/// `config.toml` の `toml::Value` から認識できないキーを収集する
///
/// 解析自体は成功させ、未知キーは警告用途に返す
/// レガシーキーや serde alias は既知として扱う
///
/// # Arguments
/// * `value` - TOML 全体の値
///
/// # Returns
/// * `Vec<String>` - ドット区切りの未知キー (辞書順)
pub(crate) fn collect_unknown_config_keys(value: &toml::Value) -> Vec<String> {
    let Some(table) = value.as_table() else {
        return Vec::new();
    };

    let mut unknown = Vec::new();
    for (key, child) in table {
        if !ROOT_KEYS.contains(&key.as_str()) {
            unknown.push(key.clone());
            continue;
        }

        match key.as_str() {
            "notification_settings" => {
                collect_table_unknown(
                    child,
                    "notification_settings",
                    NOTIFICATION_KEYS,
                    &mut unknown,
                );
            }
            "hotkeys" => {
                collect_table_unknown(child, "hotkeys", HOTKEY_KEYS, &mut unknown);
            }
            "regex" => {
                collect_table_unknown(child, "regex", REGEX_KEYS, &mut unknown);
            }
            _ => {}
        }
    }

    unknown.sort();
    unknown.dedup();
    unknown
}

/// 未知キー警告のユーザー向けメッセージを組み立てる
///
/// # Arguments
/// * `keys` - 未知キー一覧 (空であってはならない)
///
/// # Returns
/// * `String` - デスクトップ通知・CLI 向けメッセージ
#[must_use]
pub fn format_unknown_config_keys_message(keys: &[String]) -> String {
    debug_assert!(!keys.is_empty());
    let preview = format_unknown_keys_preview(keys);
    format!("認識できない設定キーがあります: {preview}。無視して読み込みました")
}

/// 未知キーをログへ記録する
pub(crate) fn warn_unknown_config_keys(keys: &[String]) {
    if keys.is_empty() {
        return;
    }
    tracing::warn!(
        "未知の設定キーを検出した: {}。無視して読み込んだ",
        keys.join(", ")
    );
}

fn collect_table_unknown(
    value: &toml::Value,
    prefix: &str,
    known: &[&str],
    unknown: &mut Vec<String>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for key in table.keys() {
        if !known.contains(&key.as_str()) {
            unknown.push(format!("{prefix}.{key}"));
        }
    }
}

fn format_unknown_keys_preview(keys: &[String]) -> String {
    if keys.len() <= UNKNOWN_KEYS_MESSAGE_LIMIT {
        return keys.join(", ");
    }
    let head = keys[..UNKNOWN_KEYS_MESSAGE_LIMIT].join(", ");
    let rest = keys.len() - UNKNOWN_KEYS_MESSAGE_LIMIT;
    format!("{head} ...他{rest}件")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 既知キーのみの TOML では未知キーが空となるようにすること
    #[test]
    fn collect_returns_empty_for_known_keys() {
        let value: toml::Value = toml::from_str(
            r#"
version = 2
mode = "Trim"
interval_ms = 1000

[notification_settings]
enabled = false
notify_mode = false

[hotkeys]
quick_selector = "Alt+Shift+S"
selector = "Alt+Shift+S"
paste_queue = "Alt+Shift+X"
favoriteModeSlots = ["Alt+Shift+1"]

[regex]
caseInsensitive = true
"#,
        )
        .expect("parse");

        assert!(collect_unknown_config_keys(&value).is_empty());
    }

    /// ルートとネストの未知キーをドット区切りで検出すること
    #[test]
    fn collect_reports_unknown_root_and_nested_keys() {
        let value: toml::Value = toml::from_str(
            r#"
version = 2
mode = "Trim"
interval_ms = 1000
typo_root = true

[hotkeys]
quick_selector = "Alt+Shift+S"
typo_hotkey = "Alt+Shift+Z"

[regex]
pattern = ""
enabled = true
"#,
        )
        .expect("parse");

        assert_eq!(
            collect_unknown_config_keys(&value),
            vec![
                "hotkeys.typo_hotkey".to_string(),
                "regex.enabled".to_string(),
                "typo_root".to_string(),
            ]
        );
    }

    /// レガシー `[[clips]]` は未知キー扱いにしないようにすること
    #[test]
    fn collect_allows_legacy_clips_array() {
        let value: toml::Value = toml::from_str(
            r#"
version = 2
mode = "Trim"
interval_ms = 1000

[[clips]]
label = "a"
text = "b"
"#,
        )
        .expect("parse");

        assert!(collect_unknown_config_keys(&value).is_empty());
    }

    /// ユーザー向けメッセージが件数制限付きで組み立てられるようにすること
    #[test]
    fn format_message_truncates_long_key_list() {
        let keys: Vec<String> = (1..=7).map(|n| format!("k{n}")).collect();
        let message = format_unknown_config_keys_message(&keys);
        assert!(message.contains("k1, k2, k3, k4, k5"));
        assert!(message.contains("...他2件"));
        assert!(message.contains("無視して読み込みました"));
        assert!(!message.contains("k6"));
    }
}
