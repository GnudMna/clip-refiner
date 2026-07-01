use std::collections::HashSet;
use std::str::FromStr;

use super::defs::{RefineCategory, RefineMode};

use clap::ValueEnum;
use strum::{EnumMessage, EnumProperty, IntoEnumIterator};

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
    pub const SUBMENU_ORDER: [Self; 14] = [
        Self::LineActions,
        Self::UrlActions,
        Self::Path,
        Self::Markdown,
        Self::Excel,
        Self::Trim,
        Self::Escape,
        Self::Regex,
        Self::JsonFormat,
        Self::ToJson,
        Self::ToYaml,
        Self::Datetime,
        Self::Number,
        Self::Case,
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
    /// # Panics
    /// バリアントに `category` 属性が未定義、または値が不正な場合
    ///
    /// # Returns
    /// * `RefineCategory` - モードが属するカテゴリ
    pub fn category(self) -> RefineCategory {
        let name = self
            .get_str("category")
            .unwrap_or_else(|| panic!("RefineMode::{self:?} に category 属性が未定義"));
        RefineCategory::from_str(name)
            .unwrap_or_else(|_| panic!("RefineMode::{self:?} の category={name:?} が不正"))
    }

    /// 画像をクリップボードへ書き込むモードかどうか
    pub fn produces_image(self) -> bool {
        matches!(self, Self::ExcelToImage)
    }

    /// クイックセレクタ向けのモード表示順を返す
    ///
    /// トレイメニューと同様に、通常項目を先頭に、続けてカテゴリ順で並べる
    ///
    /// # Returns
    /// * `Vec<RefineMode>` - 表示順に並んだモード一覧
    pub fn quick_selector_modes() -> Vec<Self> {
        let mut ordered = Vec::new();
        ordered.extend(Self::iter().filter(|m| m.category() == RefineCategory::Normal));
        for category in RefineCategory::SUBMENU_ORDER {
            ordered.extend(Self::iter().filter(|m| m.category() == category));
        }
        ordered
    }

    /// UI(Webview)に渡すためのモード情報のJSONリストを生成する
    ///
    /// 全モードをカテゴリ順で出力し、お気に入り状態と登録順序を付与する
    ///
    /// # Arguments
    /// * `favorite_modes` - お気に入り登録済みモード (登録順)
    ///
    /// # Returns
    /// * `String` - モード ID・ラベル・カテゴリ・CLI 名・お気に入り状態を含む JSON 配列文字列
    pub fn to_json_list(favorite_modes: &[Self]) -> String {
        use std::collections::HashMap;

        let favorite_set: HashSet<Self> = favorite_modes.iter().copied().collect();
        let favorite_index: HashMap<Self, usize> = favorite_modes
            .iter()
            .enumerate()
            .map(|(index, mode)| (*mode, index))
            .collect();
        let list: Vec<serde_json::Value> = Self::quick_selector_modes()
            .iter()
            .map(|m| {
                let favorite = favorite_set.contains(m);
                let mut item = serde_json::json!({
                    "id": m,
                    "label": m.label(),
                    "category": m.category().label(),
                    "value": m.to_possible_value()
                        .map(|v| v.get_name().to_string())
                        .unwrap_or_default(),
                    "favorite": favorite,
                });
                if let Some(index) = favorite_index.get(m) {
                    item["favoriteIndex"] = serde_json::json!(index);
                }
                item
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
    }

    /// 加工パイプラインの表示用ラベルを生成する
    ///
    /// # Arguments
    /// * `pipeline` - 適用順の加工モード列
    ///
    /// # Returns
    /// * `String` - モード名を ` → ` で連結した文字列
    pub fn pipeline_label(pipeline: &[Self]) -> String {
        pipeline
            .iter()
            .map(|mode| mode.label())
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::defs::{RefineCategory, RefineMode};

    use strum::{EnumProperty, IntoEnumIterator};

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

        assert_eq!(
            RefineMode::ExcelToMarkdown.category(),
            RefineCategory::Excel
        );
        assert_eq!(RefineMode::ExcelToImage.category(), RefineCategory::Excel);
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

    /// 全 `RefineMode` に category 属性が定義されていること
    #[test]
    fn test_all_refine_modes_have_category_property() {
        for mode in RefineMode::iter() {
            assert!(
                mode.get_str("category").is_some(),
                "{mode:?} に category 属性がありません"
            );
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
        let json = RefineMode::to_json_list(&[RefineMode::UrlDecode, RefineMode::Trim]);
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&json).expect("to_json_list の出力が JSON として不正");

        assert_eq!(parsed.len(), RefineMode::iter().count());

        for item in &parsed {
            assert!(item.get("id").is_some());
            assert!(item.get("label").is_some());
            assert!(item.get("category").is_some());
            assert!(item.get("value").is_some());
            assert!(item.get("favorite").is_some());
        }

        let favorites: Vec<_> = parsed
            .iter()
            .filter(|item| item.get("favorite").and_then(serde_json::Value::as_bool) == Some(true))
            .collect();
        assert_eq!(favorites.len(), 2);

        let url_decode = parsed
            .iter()
            .find(|item| item.get("id").and_then(|v| v.as_str()) == Some("UrlDecode"))
            .expect("UrlDecode が含まれる");
        assert_eq!(
            url_decode
                .get("favoriteIndex")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );

        let trim = parsed
            .iter()
            .find(|item| item.get("id").and_then(|v| v.as_str()) == Some("Trim"))
            .expect("Trim が含まれる");
        assert_eq!(
            trim.get("favoriteIndex")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );

        let json_format = parsed
            .iter()
            .find(|item| item.get("id").and_then(|v| v.as_str()) == Some("JsonFormat"))
            .expect("JsonFormat が含まれる");
        assert_eq!(
            json_format
                .get("favorite")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert!(json_format.get("favoriteIndex").is_none());
    }

    /// `quick_selector_modes` が全モードをトレイメニュー相当の順序で返すこと
    #[test]
    fn test_quick_selector_modes_order() {
        let ordered = RefineMode::quick_selector_modes();
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

    /// `ExcelToImage` がクイックセレクタに含まれること
    #[test]
    fn test_excel_to_image_in_quick_selector() {
        assert!(RefineMode::quick_selector_modes().contains(&RefineMode::ExcelToImage));
    }

    /// `RefineMode` のバリアント数と主要バリアントの存在を確認すること
    #[test]
    fn test_refine_mode_variants() {
        let variants: Vec<_> = RefineMode::iter().collect();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLinesAsc));
        assert!(variants.contains(&RefineMode::SortLinesDesc));
        assert!(variants.contains(&RefineMode::TimestampToDatetime));
        assert_eq!(variants.len(), 42);
    }
}
