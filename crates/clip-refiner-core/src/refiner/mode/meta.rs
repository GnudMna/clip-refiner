use std::collections::HashSet;

use strum::{EnumMessage, IntoEnumIterator};

use super::defs::{RefineCategory, RefineMode};

// ======================================================================
// メタデータ取得
// ======================================================================
impl RefineCategory {
    /// カテゴリの表示名を取得する
    ///
    /// # Arguments
    /// * `self` - この `RefineCategory`
    ///
    /// # Returns
    /// * `&'static str` - UIに表示するためのカテゴリ名
    pub fn label(self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// トレイメニューのサブメニュー表示順(`Normal` を除く)
    pub const SUBMENU_ORDER: [Self; 16] = [
        Self::LineActions,
        Self::UrlActions,
        Self::Base64,
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
        Self::Width,
    ];

    /// ルート直下の通常項目の後ろにサブメニューを遅延配置するカテゴリかどうか
    ///
    /// # Arguments
    /// * `self` - この `RefineCategory`
    ///
    /// # Returns
    /// * `bool` - `Datetime` または `Number` の場合は `true`
    pub fn is_deferred_in_menu(self) -> bool {
        matches!(self, Self::Datetime | Self::Number)
    }
}

impl RefineMode {
    /// UIに表示する名前を取得する
    ///
    /// # Arguments
    /// * `self` - この `RefineMode`
    ///
    /// # Returns
    /// * `&'static str` - モードに対応する静的な文字列ラベル
    pub fn label(self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// 所属するカテゴリを取得する
    ///
    /// トレイメニューの階層構築に利用される
    /// 新モード追加時は本 `match` への追加がコンパイルを要求する
    ///
    /// # Arguments
    /// * `self` - この `RefineMode`
    ///
    /// # Returns
    /// * `RefineCategory` - モードが属するカテゴリ
    pub fn category(self) -> RefineCategory {
        match self {
            Self::UrlEncode | Self::UrlDecode | Self::RemoveUtm => RefineCategory::UrlActions,
            Self::Base64Encode | Self::Base64Decode => RefineCategory::Base64,
            Self::ExtractBasename
            | Self::ExtractBasenameQuoted
            | Self::AddPathQuotes
            | Self::RemovePathQuotes
            | Self::PathToSlash
            | Self::PathToBackslash => RefineCategory::Path,
            Self::SortLinesAsc
            | Self::SortLinesDesc
            | Self::SortLinesNaturalAsc
            | Self::SortLinesNaturalDesc
            | Self::RemoveEmptyLines
            | Self::RemoveDuplicateLines => RefineCategory::LineActions,
            Self::Trim | Self::TrimLines => RefineCategory::Trim,
            Self::Escape | Self::Unescape | Self::RegexEscape | Self::RegexUnescape => {
                RefineCategory::Escape
            }
            Self::RegexReplace | Self::RegexExtract | Self::RegexDelete | Self::RegexSplit => {
                RefineCategory::Regex
            }
            Self::JsonFormat | Self::JsonFormatPreserveOrder => RefineCategory::JsonFormat,
            Self::YamlToJson | Self::YamlToJsonPreserveOrder => RefineCategory::ToJson,
            Self::JsonToYaml | Self::JsonToYamlPreserveOrder => RefineCategory::ToYaml,
            Self::MarkdownToHtml | Self::MarkdownToRichText | Self::HtmlToMarkdown => {
                RefineCategory::Markdown
            }
            Self::ExcelToMarkdown | Self::MarkdownToExcel | Self::ExcelToImage => {
                RefineCategory::Excel
            }
            Self::ToDatetimeString
            | Self::ToDatetimeStringUtc
            | Self::ToIso8601
            | Self::ToIso8601Utc
            | Self::ToRfc2822
            | Self::ToJapaneseDatetime
            | Self::ToUnixTimestamp => RefineCategory::Datetime,
            Self::AddComma | Self::RemoveComma => RefineCategory::Number,
            Self::ToCamelCase
            | Self::ToSnakeCase
            | Self::ToPascalCase
            | Self::ToKebabCase
            | Self::ToScreamingSnakeCase => RefineCategory::Case,
            Self::ToHalfwidth | Self::ToFullwidth => RefineCategory::Width,
        }
    }

    /// 画像をクリップボードへ書き込むモードかどうか
    ///
    /// # Arguments
    /// * `self` - この `RefineMode`
    ///
    /// # Returns
    /// * `bool` - 画像出力モード (`ExcelToImage` など) の場合は `true`
    pub fn produces_image(self) -> bool {
        matches!(self, Self::ExcelToImage)
    }

    /// 加工結果をリッチテキスト (`CF_HTML`) としてクリップボードへ書き込むモードかどうか
    ///
    /// パイプラインのテキスト加工の最終段にある場合のみ `CF_HTML` を書き込む
    ///
    /// # Arguments
    /// * `self` - この `RefineMode`
    ///
    /// # Returns
    /// * `bool` - `MarkdownToRichText` の場合は `true`
    pub fn produces_rich_text(self) -> bool {
        matches!(self, Self::MarkdownToRichText)
    }

    /// クリップボードの HTML 形式 (`CF_HTML`) を入力に使うモードかどうか
    ///
    /// パイプラインの先頭にある場合のみ、テキストより `CF_HTML` を優先して読む
    ///
    /// # Arguments
    /// * `self` - この `RefineMode`
    ///
    /// # Returns
    /// * `bool` - `HtmlToMarkdown` の場合は `true`
    pub fn reads_html(self) -> bool {
        matches!(self, Self::HtmlToMarkdown)
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
                    "value": m.to_string(),
                    "favorite": favorite,
                    "producesImage": m.produces_image(),
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
    /// * `pipeline` - 適用順の変換モード列
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
    use std::str::FromStr;

    use strum::IntoEnumIterator;

    use super::super::defs::{RefineCategory, RefineMode};

    /// `RefineMode` のラベルとカテゴリが期待どおりであようにすること
    #[test]
    fn test_refine_mode_metadata() {
        assert_eq!(RefineMode::UrlEncode.label(), "URLエンコード");
        assert_eq!(RefineMode::UrlEncode.category(), RefineCategory::UrlActions);

        assert_eq!(RefineMode::JsonFormat.label(), "JSON整形(キー順序ソート)");
        assert_eq!(
            RefineMode::JsonFormat.category(),
            RefineCategory::JsonFormat
        );

        assert_eq!(RefineMode::ToDatetimeString.label(), "日時→日時文字列");

        assert_eq!(
            RefineMode::ToDatetimeString.category(),
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

    /// 全モードのカテゴリが `SUBMENU_ORDER` で網羅されていようにすること
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

    /// 全 `RefineMode` の label が空でないようにすること
    #[test]
    fn test_all_refine_modes_have_nonempty_labels() {
        for mode in RefineMode::iter() {
            assert!(!mode.label().is_empty(), "{mode:?} の label が空です");
        }
    }

    /// 全 `RefineMode` がカテゴリを返し `Normal` に属さないようにすること
    ///
    /// 網羅 `match` により未定義バリアントはコンパイルエラーになる。ここは回帰用の実行時確認
    #[test]
    fn test_all_refine_modes_have_non_normal_category() {
        for mode in RefineMode::iter() {
            assert_ne!(
                mode.category(),
                RefineCategory::Normal,
                "{mode:?} は Normal カテゴリに属してはならない"
            );
        }
    }

    /// `is_deferred_in_menu` が Datetime / Number のみ true であようにすること
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

    /// `to_json_list` が全モード分の有効な JSON を返すようにすること
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
            assert!(item.get("producesImage").is_some());
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

        assert_eq!(
            url_decode.get("value").and_then(|v| v.as_str()),
            Some("url-decode")
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

    /// kebab-case の CLI 名を `FromStr` で解析できるようにすること
    #[test]
    fn test_refine_mode_from_cli_name() {
        assert_eq!(
            RefineMode::from_str("url-decode").expect("parse"),
            RefineMode::UrlDecode
        );
        assert_eq!(RefineMode::JsonFormat.to_string(), "json-format");
    }

    /// Base64 モードの CLI 名・設定値・ラベル・カテゴリが期待どおりであること
    #[test]
    fn test_base64_modes_metadata() {
        for (mode, cli, serde_name, label) in [
            (
                RefineMode::Base64Encode,
                "base64-encode",
                "Base64Encode",
                "Base64エンコード",
            ),
            (
                RefineMode::Base64Decode,
                "base64-decode",
                "Base64Decode",
                "Base64デコード",
            ),
        ] {
            assert_eq!(mode.to_string(), cli);
            assert_eq!(RefineMode::from_str(cli).expect("CLI 名"), mode);
            assert_eq!(
                serde_json::to_string(&mode).expect("シリアライズ"),
                format!("\"{serde_name}\"")
            );
            assert_eq!(mode.label(), label);
            assert_eq!(mode.category(), RefineCategory::Base64);
        }
        assert_eq!(RefineCategory::Base64.label(), "Base64");
    }

    /// 改名した日時変換モードの旧名 (CLI 名・設定値) を受け付け、書き出しは新名にすること
    #[test]
    fn test_renamed_datetime_modes_accept_old_names() {
        for (old_cli, old_serde, mode, new_cli, new_serde) in [
            (
                "timestamp-to-datetime",
                "TimestampToDatetime",
                RefineMode::ToDatetimeString,
                "to-datetime-string",
                "ToDatetimeString",
            ),
            (
                "datetime-to-timestamp",
                "DatetimeToTimestamp",
                RefineMode::ToUnixTimestamp,
                "to-unix-timestamp",
                "ToUnixTimestamp",
            ),
        ] {
            assert_eq!(RefineMode::from_str(old_cli).expect("旧 CLI 名"), mode);
            assert_eq!(RefineMode::from_str(new_cli).expect("新 CLI 名"), mode);
            assert_eq!(mode.to_string(), new_cli);

            let old: RefineMode =
                serde_json::from_str(&format!("\"{old_serde}\"")).expect("旧設定値");
            assert_eq!(old, mode);
            assert_eq!(
                serde_json::to_string(&mode).expect("シリアライズ"),
                format!("\"{new_serde}\"")
            );
        }
    }

    /// `quick_selector_modes` が全モードをトレイメニュー相当の順序で返すようにすること
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

    /// `ExcelToImage` がクイックセレクタに含まれるようにすること
    #[test]
    fn test_excel_to_image_in_quick_selector() {
        assert!(RefineMode::quick_selector_modes().contains(&RefineMode::ExcelToImage));
    }

    /// `ExcelToImage` がクイックセレクタに含まれるようにすること
    #[test]
    fn test_refine_mode_variants() {
        let variants: Vec<_> = RefineMode::iter().collect();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLinesAsc));
        assert!(variants.contains(&RefineMode::SortLinesDesc));
        assert!(variants.contains(&RefineMode::SortLinesNaturalAsc));
        assert!(variants.contains(&RefineMode::SortLinesNaturalDesc));
        assert!(variants.contains(&RefineMode::ToDatetimeString));
        assert_eq!(variants.len(), 55);
    }
}
