use crate::config::{AppConfig, MonitorMode, NotificationSettings, RegexSettings};
use crate::platform::autostart;
use crate::refiner::RefineMode;

use serde::{Deserialize, Serialize};

// ======================================================================
// 設定モデル
// ======================================================================
macro_rules! define_settings_model {
    (
        $(
            $field:ident : $ty:ty
        ),* $(,)?
    ) => {
        /// 設定ウィンドウで読み書きする項目
        ///
        /// `SettingsSnapshot` は `#[serde(flatten)]` で埋め込み、
        /// `SettingsPatch` は同じフィールドを `Option` で部分更新する
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct SettingsWritable {
            $( pub $field : $ty, )*
        }

        /// 設定ウィンドウから適用する部分更新
        #[derive(Debug, Clone, Default, Deserialize, Serialize)]
        #[serde(rename_all = "camelCase", default)]
        pub(crate) struct SettingsPatch {
            $( pub $field : Option<$ty>, )*
        }

        impl SettingsWritable {
            /// 部分更新をメモリ上の表示用状態へ反映する
            ///
            /// # Arguments
            /// * `patch` - 適用する部分更新
            #[cfg_attr(not(test), allow(dead_code))]
            pub fn apply_patch(&mut self, patch: &SettingsPatch) {
                $(
                    if let Some(value) = patch.$field.clone() {
                        self.$field = value;
                    }
                )*
            }
        }
    };
}

define_settings_model! {
    monitor_mode: MonitorMode,
    interval_ms: u64,
    mode: RefineMode,
    pipeline: Vec<RefineMode>,
    notification_settings: NotificationSettings,
    history_enabled: bool,
    history_limit: usize,
    continuous_record_enabled: bool,
    continuous_record_limit: usize,
    regex: RegexSettings,
    launch_at_login: bool,
    is_paused: bool,
}

impl SettingsWritable {
    /// `AppConfig` と OS 状態から書き込み可能項目を構築する
    ///
    /// # Arguments
    /// * `config` - 現在のアプリケーション設定
    ///
    /// # Returns
    /// * `Self` - 設定ウィンドウ向けの書き込み可能項目
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            monitor_mode: config.monitor_mode,
            interval_ms: config.interval_ms,
            mode: config.mode,
            pipeline: config.pipeline.clone(),
            notification_settings: config.notification_settings.clone(),
            history_enabled: config.history_enabled,
            history_limit: config.history_limit,
            continuous_record_enabled: config.continuous_record_enabled,
            continuous_record_limit: config.continuous_record_limit,
            regex: config.regex.clone(),
            launch_at_login: autostart::is_enabled(),
            is_paused: config.is_paused,
        }
    }
}

impl SettingsPatch {
    /// 後から届いた部分更新を既存パッチへマージする
    ///
    /// ネスト項目 (`notification_settings` / `regex`) は JSON 単位で深くマージする
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn merge_from(&mut self, other: &Self) {
        let Ok(mut base_json) = serde_json::to_value(&*self) else {
            return;
        };
        let Ok(other_json) = serde_json::to_value(other) else {
            return;
        };
        deep_merge_json_values(&mut base_json, &other_json);
        if let Ok(merged) = serde_json::from_value(base_json) {
            *self = merged;
        }
    }
}

/// JSON オブジェクトを再帰的にマージする (`null` はスキップ)
pub(crate) fn deep_merge_json_values(base: &mut serde_json::Value, other: &serde_json::Value) {
    let (Some(base_map), Some(other_map)) = (base.as_object_mut(), other.as_object()) else {
        if !other.is_null() {
            *base = other.clone();
        }
        return;
    };

    for (key, other_value) in other_map {
        if other_value.is_null() {
            continue;
        }
        match base_map.get_mut(key) {
            Some(base_value) if base_value.is_object() && other_value.is_object() => {
                deep_merge_json_values(base_value, other_value);
            }
            _ => {
                base_map.insert(key.clone(), other_value.clone());
            }
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::AppConfig;
    use crate::refiner::RefineMode;

    /// `SettingsPatch` の各キーが `SettingsWritable` と対応すること
    #[test]
    fn patch_json_keys_match_writable() {
        let writable = SettingsWritable::from_config(&AppConfig::default());
        let writable_json: serde_json::Value =
            serde_json::to_value(&writable).expect("SettingsWritable の JSON 化");
        let patch_json: serde_json::Value =
            serde_json::to_value(SettingsPatch::default()).expect("SettingsPatch の JSON 化");

        let writable_keys = writable_json
            .as_object()
            .expect("SettingsWritable はオブジェクト")
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        for key in patch_json
            .as_object()
            .expect("SettingsPatch はオブジェクト")
            .keys()
        {
            assert!(
                writable_keys.contains(key),
                "SettingsPatch のキー {key} が SettingsWritable に無い"
            );
        }
    }

    /// `apply_patch` が指定フィールドのみ更新すること
    #[test]
    fn writable_apply_patch_updates_selected_fields() {
        let mut writable = SettingsWritable::from_config(&AppConfig::default());
        let original_interval = writable.interval_ms;

        writable.apply_patch(&SettingsPatch {
            history_enabled: Some(true),
            mode: Some(RefineMode::Trim),
            ..SettingsPatch::default()
        });

        assert!(writable.history_enabled);
        assert_eq!(writable.mode, RefineMode::Trim);
        assert_eq!(writable.interval_ms, original_interval);
    }
}
