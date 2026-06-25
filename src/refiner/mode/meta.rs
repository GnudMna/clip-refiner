use clap::ValueEnum;
use strum::{EnumMessage, IntoEnumIterator};

use super::defs::{RefineCategory, RefineMode};

// ======================================================================
// メタデータ取得
// ======================================================================
impl RefineCategory {
    /// カテゴリの表示名を取得する
    ///
    /// # Returns
    /// * `&'static str` - UIに表示するためのカテゴリ名
    pub fn label(self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// トレイメニューのサブメニュー表示順(`Normal` を除く)
    pub const SUBMENU_ORDER: [Self; 12] = [
        Self::LineActions,
        Self::UrlActions,
        Self::Path,
        Self::Markdown,
        Self::Trim,
        Self::Escape,
        Self::Regex,
        Self::JsonFormat,
        Self::ToJson,
        Self::ToYaml,
        Self::Datetime,
        Self::Number,
    ];

    /// ルート直下の通常項目の後ろにサブメニューを遅延配置するカテゴリかどうか
    pub fn is_deferred_in_menu(self) -> bool {
        matches!(self, Self::Datetime | Self::Number)
    }
}

impl RefineMode {
    /// UIに表示する名前を取得する
    ///
    /// # Returns
    /// * `&'static str` - モードに対応する静的な文字列ラベル
    pub fn label(self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// 所属するカテゴリを取得する。トレイメニューの階層構築に利用される
    ///
    /// # Returns
    /// * `RefineCategory` - モードが属するカテゴリ
    pub fn category(self) -> RefineCategory {
        use RefineCategory as C;
        match self {
            Self::UrlEncode | Self::UrlDecode | Self::RemoveUtm => C::UrlActions,
            Self::ExtractBasename
            | Self::ExtractBasenameQuoted
            | Self::AddPathQuotes
            | Self::RemovePathQuotes
            | Self::PathToSlash
            | Self::PathToBackslash => C::Path,
            Self::SortLinesAsc
            | Self::SortLinesDesc
            | Self::RemoveEmptyLines
            | Self::RemoveDuplicateLines => C::LineActions,
            Self::Trim | Self::TrimLines => C::Trim,
            Self::Escape | Self::Unescape | Self::RegexEscape | Self::RegexUnescape => C::Escape,
            Self::RegexReplace | Self::RegexExtract | Self::RegexDelete | Self::RegexSplit => {
                C::Regex
            }
            Self::JsonFormat | Self::JsonFormatPreserveOrder => C::JsonFormat,
            Self::YamlToJson | Self::YamlToJsonPreserveOrder => C::ToJson,
            Self::JsonToYaml | Self::JsonToYamlPreserveOrder => C::ToYaml,
            Self::MarkdownToHtml | Self::ExcelToMarkdown | Self::MarkdownToExcel => C::Markdown,
            Self::TimestampToDatetime | Self::DatetimeToTimestamp => C::Datetime,
            Self::AddComma | Self::RemoveComma => C::Number,
        }
    }

    /// クイックセレクタ向けのモード表示順を返す
    ///
    /// トレイメニューと同様に、通常項目を先頭に、続けてカテゴリ順で並べる
    ///
    /// # Returns
    /// * `Vec<RefineMode>` - 表示順に並んだモード一覧
    pub fn selector_modes() -> Vec<Self> {
        let mut ordered = Vec::new();
        ordered.extend(Self::iter().filter(|m| m.category() == RefineCategory::Normal));
        for category in RefineCategory::SUBMENU_ORDER {
            ordered.extend(Self::iter().filter(|m| m.category() == category));
        }
        ordered
    }

    /// UI(Webview)に渡すためのモード情報のJSONリストを生成する
    ///
    /// # Returns
    /// * `String` - モード ID・ラベル・カテゴリ・CLI 名を含む JSON 配列文字列
    pub fn to_json_list() -> String {
        let list: Vec<serde_json::Value> = Self::selector_modes()
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m,
                    "label": m.label(),
                    "category": m.category().label(),
                    "value": m.to_possible_value()
                        .map(|v| v.get_name().to_string())
                        .unwrap_or_default(),
                })
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::super::defs::{RefineCategory, RefineMode};

    /// `RefineMode` のラベルとカテゴリが期待どおりであること
    #[test]
    fn test_refine_mode_metadata() {
        assert_eq!(RefineMode::UrlEncode.label(), "URLエンコード");
        assert_eq!(RefineMode::UrlEncode.category(), RefineCategory::UrlActions);

        assert_eq!(RefineMode::JsonFormat.label(), "JSON整形(キー順序不同)");
        assert_eq!(
            RefineMode::JsonFormat.category(),
            RefineCategory::JsonFormat
        );

        assert_eq!(
            RefineMode::TimestampToDatetime.label(),
            "Unixタイムスタンプ→日時文字列"
        );

        assert_eq!(
            RefineMode::TimestampToDatetime.category(),
            RefineCategory::Datetime
        );

        assert_eq!(
            RefineMode::MarkdownToHtml.category(),
            RefineCategory::Markdown
        );
    }

    /// 全モードのカテゴリが `SUBMENU_ORDER` で網羅されていること
    #[test]
    fn test_submenu_order_covers_all_categories() {
        use std::collections::HashSet;

        let used: HashSet<_> = RefineMode::iter()
            .map(RefineMode::category)
            .filter(|c| *c != RefineCategory::Normal)
            .collect();
        let ordered: HashSet<_> = RefineCategory::SUBMENU_ORDER.into_iter().collect();

        assert_eq!(
            used, ordered,
            "RefineCategory::SUBMENU_ORDER が全サブメニューカテゴリを網羅していません"
        );
    }

    /// 全 `RefineMode` の label が空でないこと
    #[test]
    fn test_all_refine_modes_have_nonempty_labels() {
        for mode in RefineMode::iter() {
            assert!(!mode.label().is_empty(), "{mode:?} の label が空です");
        }
    }

    /// `is_deferred_in_menu` が Datetime / Number のみ true であること
    #[test]
    fn test_refine_category_is_deferred_in_menu() {
        use RefineCategory::*;

        assert!(Datetime.is_deferred_in_menu());
        assert!(Number.is_deferred_in_menu());

        for category in RefineCategory::iter() {
            if matches!(category, Datetime | Number) {
                continue;
            }
            assert!(
                !category.is_deferred_in_menu(),
                "{category:?} は遅延配置対象ではない"
            );
        }
    }

    /// `to_json_list` が全モード分の有効な JSON を返すこと
    #[test]
    fn test_to_json_list() {
        let json = RefineMode::to_json_list();
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&json).expect("to_json_list の出力が JSON として不正");

        assert_eq!(parsed.len(), RefineMode::iter().count());

        for item in parsed {
            assert!(item.get("id").is_some());
            assert!(item.get("label").is_some());
            assert!(item.get("category").is_some());
            assert!(item.get("value").is_some());
        }
    }

    /// `selector_modes` が全モードをトレイメニュー相当の順序で返すこと
    #[test]
    fn test_selector_modes_order() {
        let ordered = RefineMode::selector_modes();
        assert_eq!(ordered.len(), RefineMode::iter().count());

        let normal_count = RefineMode::iter()
            .filter(|m| m.category() == RefineCategory::Normal)
            .count();
        for mode in &ordered[..normal_count] {
            assert_eq!(mode.category(), RefineCategory::Normal);
        }

        let mut seen_categories = Vec::new();
        for mode in ordered.iter().skip(normal_count) {
            let category = mode.category();
            if seen_categories.last() != Some(&category) {
                seen_categories.push(category);
            }
        }
        assert_eq!(seen_categories, RefineCategory::SUBMENU_ORDER.to_vec());
    }

    /// `RefineMode` のバリアント数と主要バリアントの存在を確認すること
    #[test]
    fn test_refine_mode_variants() {
        let variants: Vec<_> = RefineMode::iter().collect();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLinesAsc));
        assert!(variants.contains(&RefineMode::SortLinesDesc));
        assert!(variants.contains(&RefineMode::TimestampToDatetime));
        assert_eq!(variants.len(), 36);
    }
}
