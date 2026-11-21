use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::consts::{
    DEFAULT_REGEX_PRESET_ID, DEFAULT_REGEX_PRESET_NAME, MAX_REGEX_PRESETS,
    MAX_REGEX_STEPS_PER_PRESET,
};

// ======================================================================
// 正規表現ステップ
// ======================================================================
/// プリセット内の 1 ステップ分のパターンと置換文字列
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegexStep {
    /// 正規表現パターン
    #[serde(default)]
    pub pattern: String,
    /// 置換文字列 (`RegexReplace` で使用。キャプチャグループは `$1` 形式)
    #[serde(default)]
    pub replacement: String,
    /// 大文字小文字を無視する (`(?i)` 相当)
    #[serde(
        default,
        rename(serialize = "caseInsensitive", deserialize = "case_insensitive"),
        alias = "caseInsensitive"
    )]
    pub case_insensitive: bool,
    /// 複数行モード (`(?m)` 相当)
    #[serde(default)]
    pub multiline: bool,
}

// ======================================================================
// 正規表現プリセット
// ======================================================================
/// 名前付きの正規表現ステップ列
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegexPreset {
    /// プリセット識別子 (設定内で一意)
    #[serde(default)]
    pub id: String,
    /// 表示名
    #[serde(default)]
    pub name: String,
    /// 上から順に適用するステップ
    #[serde(default)]
    pub steps: Vec<RegexStep>,
}

impl Default for RegexPreset {
    fn default() -> Self {
        Self {
            id: DEFAULT_REGEX_PRESET_ID.to_string(),
            name: DEFAULT_REGEX_PRESET_NAME.to_string(),
            steps: vec![RegexStep::default()],
        }
    }
}

// ======================================================================
// 正規表現設定
// ======================================================================
/// 正規表現変換モード用のプリセット一覧
///
/// `config.toml` の `[regex]` セクションとして保存される
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RegexSettings {
    /// 有効なプリセットの `id`
    #[serde(rename(serialize = "activePresetId"))]
    pub active_preset_id: String,
    /// 名前付きプリセット一覧
    pub presets: Vec<RegexPreset>,
}

impl Default for RegexSettings {
    fn default() -> Self {
        Self {
            active_preset_id: DEFAULT_REGEX_PRESET_ID.to_string(),
            presets: vec![RegexPreset::default()],
        }
    }
}

impl RegexSettings {
    /// 単一ステップから設定を構築する
    ///
    /// # Arguments
    /// * `step` - 唯一のステップ
    ///
    /// # Returns
    /// * `Self` - デフォルトプリセット 1 件の設定
    pub fn from_step(step: RegexStep) -> Self {
        Self {
            active_preset_id: DEFAULT_REGEX_PRESET_ID.to_string(),
            presets: vec![RegexPreset {
                id: DEFAULT_REGEX_PRESET_ID.to_string(),
                name: DEFAULT_REGEX_PRESET_NAME.to_string(),
                steps: vec![step],
            }],
        }
    }

    /// パターンと置換文字列だけの単一ステップ設定を構築する
    ///
    /// # Arguments
    /// * `pattern` - 正規表現パターン
    /// * `replacement` - 置換文字列
    ///
    /// # Returns
    /// * `Self` - デフォルトプリセット 1 件の設定
    pub fn single(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self::from_step(RegexStep {
            pattern: pattern.into(),
            replacement: replacement.into(),
            ..RegexStep::default()
        })
    }

    /// 旧フラット `[regex]` からプリセット 1 件へ包む
    ///
    /// # Arguments
    /// * `step` - 旧形式のフィールド相当
    ///
    /// # Returns
    /// * `Self` - 移行後の設定
    pub fn from_legacy_flat(step: RegexStep) -> Self {
        Self::from_step(step)
    }

    /// 有効プリセットを返す (不正 ID のときは先頭)
    ///
    /// # Returns
    /// * `Option<&RegexPreset>` - 有効プリセット。プリセットが無いときは `None`
    pub fn active_preset(&self) -> Option<&RegexPreset> {
        if let Some(preset) = self
            .presets
            .iter()
            .find(|preset| preset.id == self.active_preset_id)
        {
            return Some(preset);
        }
        self.presets.first()
    }

    /// 有効プリセットのステップ列を返す
    ///
    /// # Returns
    /// * `&[RegexStep]` - 有効プリセットのステップ。プリセットが無いときは空
    pub fn active_steps(&self) -> &[RegexStep] {
        self.active_preset()
            .map_or(&[], |preset| preset.steps.as_slice())
    }

    /// 名前が一致する先頭プリセットを有効化する
    ///
    /// # Arguments
    /// * `name` - プリセット表示名
    ///
    /// # Returns
    /// * `bool` - 一致するプリセットがあったとき `true`
    pub fn select_preset_by_name(&mut self, name: &str) -> bool {
        if let Some(preset) = self.presets.iter().find(|preset| preset.name == name) {
            self.active_preset_id = preset.id.clone();
            true
        } else {
            false
        }
    }

    /// ID が一致するプリセットを有効化する
    ///
    /// # Arguments
    /// * `id` - プリセット ID
    ///
    /// # Returns
    /// * `bool` - 有効 ID が変わったとき `true` (未登録 ID や既に有効な場合は `false`)
    pub fn select_preset_by_id(&mut self, id: &str) -> bool {
        if self.active_preset_id == id {
            return false;
        }
        if self.presets.iter().any(|preset| preset.id == id) {
            self.active_preset_id = id.to_string();
            true
        } else {
            false
        }
    }

    /// CLI / IPC 向けに有効プリセットを単一ステップへ置き換える
    ///
    /// 未指定フィールドは現在の有効プリセット先頭ステップ (無ければ既定) を引き継ぐ
    /// `--regex-pattern` など 1 ステップ上書き専用。複数ステップの委譲は
    /// [`Self::apply_oneshot_steps_override`] を使う
    ///
    /// # Arguments
    /// * `pattern` - 上書きするパターン
    /// * `replacement` - 上書きする置換文字列
    /// * `case_insensitive` - 上書きする CI フラグ
    /// * `multiline` - 上書きする ML フラグ
    pub fn apply_oneshot_step_override(
        &mut self,
        pattern: Option<String>,
        replacement: Option<String>,
        case_insensitive: Option<bool>,
        multiline: Option<bool>,
    ) {
        let mut step = self.active_steps().first().cloned().unwrap_or_default();
        if let Some(pattern) = pattern {
            step.pattern = pattern;
        }
        if let Some(replacement) = replacement {
            step.replacement = replacement;
        }
        if let Some(case_insensitive) = case_insensitive {
            step.case_insensitive = case_insensitive;
        }
        if let Some(multiline) = multiline {
            step.multiline = multiline;
        }
        self.replace_active_preset_steps(None, vec![step]);
    }

    /// CLI / IPC 向けに有効プリセットを指定ステップ列へ置き換える
    ///
    /// 空のステップ列は既定の 1 ステップにする。件数は [`MAX_REGEX_STEPS_PER_PRESET`] で打ち切る
    ///
    /// # Arguments
    /// * `name` - 置き換え後プリセットの表示名。`None` または空なら現在の名前を維持する
    /// * `steps` - 適用するステップ列
    pub fn apply_oneshot_steps_override(&mut self, name: Option<&str>, steps: &[RegexStep]) {
        self.replace_active_preset_steps(name, steps.to_vec());
    }

    /// 有効プリセットを 1 件だけ残し、ステップ列を置き換える
    ///
    /// # Arguments
    /// * `name` - 置き換え後プリセットの表示名。`None` または空なら現在の名前を維持する
    /// * `steps` - 適用するステップ列
    fn replace_active_preset_steps(&mut self, name: Option<&str>, mut steps: Vec<RegexStep>) {
        if steps.is_empty() {
            steps.push(RegexStep::default());
        }
        steps.truncate(MAX_REGEX_STEPS_PER_PRESET);

        let id = if self.active_preset_id.is_empty() {
            DEFAULT_REGEX_PRESET_ID.to_string()
        } else {
            self.active_preset_id.clone()
        };
        let name = name.filter(|value| !value.is_empty()).map_or_else(
            || {
                self.active_preset().map_or_else(
                    || DEFAULT_REGEX_PRESET_NAME.to_string(),
                    |preset| preset.name.clone(),
                )
            },
            str::to_string,
        );

        self.active_preset_id.clone_from(&id);
        self.presets = vec![RegexPreset { id, name, steps }];
    }

    /// 件数上限・空配列・不正 ID を補正する
    pub fn normalize(&mut self) {
        if self.presets.is_empty() {
            *self = Self::default();
            return;
        }

        self.presets.truncate(MAX_REGEX_PRESETS);
        for (index, preset) in self.presets.iter_mut().enumerate() {
            if preset.id.is_empty() {
                preset.id = format!("preset-{index}");
            }
            if preset.name.is_empty() {
                preset.name = format!("プリセット {}", index + 1);
            }
            if preset.steps.is_empty() {
                preset.steps.push(RegexStep::default());
            }
            preset.steps.truncate(MAX_REGEX_STEPS_PER_PRESET);
        }

        if !self
            .presets
            .iter()
            .any(|preset| preset.id == self.active_preset_id)
        {
            self.active_preset_id = self.presets[0].id.clone();
        }
    }
}

// ======================================================================
// 後方互換デシリアライズ (フラット / プリセット)
// ======================================================================
impl<'de> Deserialize<'de> for RegexSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RegexSettingsVisitor)
    }
}

struct RegexSettingsVisitor;

impl<'de> Visitor<'de> for RegexSettingsVisitor {
    type Value = RegexSettings;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("regex settings table")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut active_preset_id: Option<String> = None;
        let mut presets: Option<Vec<RegexPreset>> = None;
        let mut pattern: Option<String> = None;
        let mut replacement: Option<String> = None;
        let mut case_insensitive: Option<bool> = None;
        let mut multiline: Option<bool> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "active_preset_id" | "activePresetId" => {
                    active_preset_id = Some(map.next_value()?);
                }
                "presets" => {
                    presets = Some(map.next_value()?);
                }
                "pattern" => {
                    pattern = Some(map.next_value()?);
                }
                "replacement" => {
                    replacement = Some(map.next_value()?);
                }
                "case_insensitive" | "caseInsensitive" => {
                    case_insensitive = Some(map.next_value()?);
                }
                "multiline" => {
                    multiline = Some(map.next_value()?);
                }
                _ => {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        if let Some(presets) = presets {
            let mut settings = RegexSettings {
                active_preset_id: active_preset_id
                    .unwrap_or_else(|| DEFAULT_REGEX_PRESET_ID.to_string()),
                presets,
            };
            settings.normalize();
            return Ok(settings);
        }

        if pattern.is_some()
            || replacement.is_some()
            || case_insensitive.is_some()
            || multiline.is_some()
        {
            return Ok(RegexSettings::from_legacy_flat(RegexStep {
                pattern: pattern.unwrap_or_default(),
                replacement: replacement.unwrap_or_default(),
                case_insensitive: case_insensitive.unwrap_or(false),
                multiline: multiline.unwrap_or(false),
            }));
        }

        Ok(RegexSettings::default())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 旧フラット形式をプリセット 1 件へ読み込めるようにすること
    #[cfg(feature = "config")]
    #[test]
    fn deserialize_legacy_flat_regex() {
        let settings: RegexSettings = toml::from_str(
            r#"
pattern = "\\d+"
replacement = "X"
case_insensitive = true
multiline = false
"#,
        )
        .expect("legacy regex");

        assert_eq!(settings.presets.len(), 1);
        assert_eq!(settings.active_steps()[0].pattern, r"\d+");
        assert_eq!(settings.active_steps()[0].replacement, "X");
        assert!(settings.active_steps()[0].case_insensitive);
    }

    /// プリセット形式を読み込めるようにすること
    #[cfg(feature = "config")]
    #[test]
    fn deserialize_preset_regex() {
        let settings: RegexSettings = toml::from_str(
            r#"
active_preset_id = "p1"
presets = [
  { id = "p1", name = "A", steps = [
    { pattern = "a", replacement = "b", case_insensitive = false, multiline = true },
  ]},
]
"#,
        )
        .expect("preset regex");

        assert_eq!(settings.active_preset_id, "p1");
        assert_eq!(settings.active_steps().len(), 1);
        assert!(settings.active_steps()[0].multiline);
    }

    /// 不正な `active_preset_id` は先頭プリセットへフォールバックすること
    #[test]
    fn active_preset_falls_back_to_first() {
        let settings = RegexSettings {
            active_preset_id: "missing".to_string(),
            presets: vec![
                RegexPreset {
                    id: "a".to_string(),
                    name: "A".to_string(),
                    steps: vec![RegexStep {
                        pattern: "1".to_string(),
                        ..RegexStep::default()
                    }],
                },
                RegexPreset {
                    id: "b".to_string(),
                    name: "B".to_string(),
                    steps: vec![RegexStep {
                        pattern: "2".to_string(),
                        ..RegexStep::default()
                    }],
                },
            ],
        };
        assert_eq!(settings.active_preset().unwrap().id, "a");
        assert_eq!(settings.active_steps()[0].pattern, "1");
    }

    /// ID 指定で有効プリセットを切り替え、同一 ID では変更なしとすること
    #[test]
    fn select_preset_by_id_changes_only_when_needed() {
        let mut settings = RegexSettings {
            active_preset_id: "a".to_string(),
            presets: vec![
                RegexPreset {
                    id: "a".to_string(),
                    name: "A".to_string(),
                    steps: vec![RegexStep::default()],
                },
                RegexPreset {
                    id: "b".to_string(),
                    name: "B".to_string(),
                    steps: vec![RegexStep::default()],
                },
            ],
        };

        assert!(!settings.select_preset_by_id("a"));
        assert!(settings.select_preset_by_id("b"));
        assert_eq!(settings.active_preset_id, "b");
        assert!(!settings.select_preset_by_id("missing"));
        assert_eq!(settings.active_preset_id, "b");
    }

    /// ワンショットの 1 ステップ上書きは有効プリセットを先頭 1 件へ畳むこと
    #[test]
    fn apply_oneshot_step_override_collapses_to_single_step() {
        let mut settings = RegexSettings {
            active_preset_id: "a".to_string(),
            presets: vec![RegexPreset {
                id: "a".to_string(),
                name: "日付".to_string(),
                steps: vec![
                    RegexStep {
                        pattern: "1".to_string(),
                        replacement: "a".to_string(),
                        ..RegexStep::default()
                    },
                    RegexStep {
                        pattern: "2".to_string(),
                        replacement: "b".to_string(),
                        ..RegexStep::default()
                    },
                ],
            }],
        };

        settings.apply_oneshot_step_override(Some("x".into()), None, None, None);

        assert_eq!(settings.presets.len(), 1);
        assert_eq!(
            settings.active_preset().map(|preset| preset.name.as_str()),
            Some("日付")
        );
        assert_eq!(settings.active_steps().len(), 1);
        assert_eq!(settings.active_steps()[0].pattern, "x");
        assert_eq!(settings.active_steps()[0].replacement, "a");
    }

    /// ワンショットのステップ列上書きで有効プリセットを全件置き換えること
    #[test]
    fn apply_oneshot_steps_override_replaces_all_steps() {
        let mut settings = RegexSettings::default();
        let steps = vec![
            RegexStep {
                pattern: "a".to_string(),
                replacement: "A".to_string(),
                ..RegexStep::default()
            },
            RegexStep {
                pattern: "b".to_string(),
                replacement: "B".to_string(),
                case_insensitive: true,
                ..RegexStep::default()
            },
        ];

        settings.apply_oneshot_steps_override(Some("日付"), &steps);

        assert_eq!(settings.presets.len(), 1);
        assert_eq!(
            settings.active_preset().map(|preset| preset.name.as_str()),
            Some("日付")
        );
        assert_eq!(settings.active_steps(), steps.as_slice());
    }

    /// ステップ列上書きは上限件数で打ち切ること
    #[test]
    fn apply_oneshot_steps_override_truncates_to_max() {
        let mut settings = RegexSettings::default();
        let steps = vec![RegexStep::default(); MAX_REGEX_STEPS_PER_PRESET + 2];
        settings.apply_oneshot_steps_override(None, &steps);
        assert_eq!(settings.active_steps().len(), MAX_REGEX_STEPS_PER_PRESET);
    }

    /// 空のステップ列上書きは既定の 1 ステップになること
    #[test]
    fn apply_oneshot_steps_override_empty_becomes_default_step() {
        let mut settings = RegexSettings::default();
        settings.apply_oneshot_steps_override(None, &[]);

        assert_eq!(settings.active_steps().len(), 1);
        assert_eq!(settings.active_steps()[0], RegexStep::default());
    }
}
