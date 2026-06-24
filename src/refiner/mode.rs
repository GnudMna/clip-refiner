use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumMessage, IntoEnumIterator, IntoStaticStr};

// ======================================================================
// 加工モード定義
// ======================================================================
/// クリップボードのテキストを加工する各モードの定義
///
/// 各バリアントは特定のテキスト加工処理(エンコード、デコード、整形、変換など)に対応している
/// `strum` マクロを使用して UI 表示用のラベルを保持している
/// カテゴリへの所属は `RefineMode::category()` で定義する
#[derive(
    Copy,
    Clone,
    Debug,
    ValueEnum,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    EnumIter,
    EnumMessage,
    IntoStaticStr,
)]
pub enum RefineMode {
    /// URLエンコードを行う
    #[value(help = "URLエンコード")]
    #[strum(message = "URLエンコード")]
    UrlEncode,
    /// URLデコードを行う。失敗した場合は元のテキストを維持する
    #[value(help = "URLデコード")]
    #[strum(message = "URLデコード")]
    UrlDecode,
    /// URLから utm_ で始まる計測用パラメータを削除する
    #[value(help = "UTMパラメータを削除")]
    #[strum(message = "UTM除去")]
    RemoveUtm,
    /// パスからベースネームを抽出する
    #[value(help = "パスからベースネームを抽出")]
    #[strum(message = "ベースネーム抽出")]
    ExtractBasename,
    /// パスからベースネームを抽出しダブルクォーテーションで囲む
    #[value(help = "パスからベースネームを抽出(引用符付き)")]
    #[strum(message = "ベースネーム抽出(引用符付)")]
    ExtractBasenameQuoted,
    /// パスの前後にダブルクォーテーションを付与する
    #[value(help = "パスに引用符を付与")]
    #[strum(message = "引用符を付与")]
    AddPathQuotes,
    /// パスの前後にあるダブルクォーテーションを削除する
    #[value(help = "パスの引用符を削除")]
    #[strum(message = "引用符を削除")]
    RemovePathQuotes,
    /// パスのバックスラッシュをスラッシュに変換する
    #[value(help = "パスをスラッシュ区切りに変換")]
    #[strum(message = "スラッシュ区切りに変換")]
    PathToSlash,
    /// パスのスラッシュをバックスラッシュに変換する
    #[value(help = "パスをバックスラッシュ区切りに変換")]
    #[strum(message = "バックスラッシュ区切りに変換")]
    PathToBackslash,
    /// 行単位で昇順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "昇順で並び替え")]
    #[strum(message = "昇順で並び替え")]
    SortLinesAsc,
    /// 行単位で降順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "降順で並び替え")]
    #[strum(message = "降順で並び替え")]
    SortLinesDesc,
    /// 空行を削除する
    #[value(help = "空行を削除")]
    #[strum(message = "空行削除")]
    RemoveEmptyLines,
    /// 重複行を削除する
    #[value(help = "重複行を削除")]
    #[strum(message = "重複行削除")]
    RemoveDuplicateLines,
    /// テキスト全体の前後にある空白および改行を削除する
    #[value(help = "改行や空白を整形")]
    #[strum(message = "全体をトリム")]
    Trim,
    /// 行ごとに前後の空白を削除する
    #[value(help = "行単位で改行や空白を整形")]
    #[strum(message = "行単位でトリム")]
    TrimLines,
    /// 文字列をバックスラッシュでエスケープする
    #[value(help = "文字列をエスケープ")]
    #[strum(message = "エスケープ")]
    Escape,
    /// 文字列のエスケープを解除する
    #[value(help = "文字列のアンエスケープ")]
    #[strum(message = "アンエスケープ")]
    Unescape,
    /// 正規表現のメタ文字をエスケープする
    #[value(help = "正規表現のエスケープ")]
    #[strum(message = "正規表現エスケープ")]
    RegexEscape,
    /// 正規表現のエスケープを解除する
    #[value(help = "正規表現のアンエスケープ")]
    #[strum(message = "正規表現アンエスケープ")]
    RegexUnescape,
    /// JSON形式をインデント整形する(キーの順序はパース時に不定となる)
    #[value(help = "JSON形式を整形(キー順序不同)")]
    #[strum(message = "JSON整形(キー順序不同)")]
    JsonFormat,
    /// JSON形式をインデント整形する(元のキー順序を維持する)
    #[value(help = "JSON形式を整形(キー順序保持)")]
    #[strum(message = "JSON整形(キー順序保持)")]
    JsonFormatPreserveOrder,
    /// YAML形式をJSON形式へ変換する
    #[value(help = "YAML形式をJSON形式へ変換(キー順序不同)")]
    #[strum(message = "YAML→JSON(キー順序不同)")]
    YamlToJson,
    /// YAML形式をJSON形式へ変換する(元のキー順序を維持する)
    #[value(help = "YAML形式をJSON形式へ変換(キー順序保持)")]
    #[strum(message = "YAML→JSON(キー順序保持)")]
    YamlToJsonPreserveOrder,
    /// JSON形式をYAML形式へ変換する
    #[value(help = "JSON形式をYAML形式へ変換(キー順序不同)")]
    #[strum(message = "JSON→YAML(キー順序不同)")]
    JsonToYaml,
    /// JSON形式をYAML形式へ変換する(元のキー順序を維持する)
    #[value(help = "JSON形式をYAML形式へ変換(キー順序保持)")]
    #[strum(message = "JSON→YAML(キー順序保持)")]
    JsonToYamlPreserveOrder,
    /// `Markdown形式のテキストをHTML形式へ変換する`
    #[value(help = "MarkdownをHTML形式へ変換")]
    #[strum(message = "Markdown→HTML")]
    MarkdownToHtml,
    /// `ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する`
    #[value(help = "Excel(TSV)をMarkdown形式へ変換")]
    #[strum(message = "Excel→Markdown")]
    ExcelToMarkdown,
    /// `Markdown形式の表をExcel(TSV)形式へ変換する`
    #[value(help = "Markdown表をExcel(TSV)形式へ変換")]
    #[strum(message = "Markdown→Excel")]
    MarkdownToExcel,
    /// Unixタイムスタンプを日時文字列へ変換する
    #[value(help = "Unixタイムスタンプを日時文字列へ変換")]
    #[strum(message = "Unixタイムスタンプ→日時文字列")]
    TimestampToDatetime,
    /// 日時文字列をUnixタイムスタンプへ変換する
    #[value(help = "日時文字列をUnixタイムスタンプへ変換")]
    #[strum(message = "日時文字列→Unixタイムスタンプ")]
    DatetimeToTimestamp,
    /// 数値に対して3桁ごとのカンマを付与する(例: 1000 -> 1,000)
    #[value(help = "カンマ無し数値をカンマ区切りの数値に")]
    #[strum(message = "カンマ追加")]
    AddComma,
    /// 数値からカンマを削除する(例: 1,000 -> 1000)
    #[value(help = "カンマ区切りの数値をカンマ無し数値に")]
    #[strum(message = "カンマ除去")]
    RemoveComma,
}

// ======================================================================
// カテゴリ定義
// ======================================================================
/// トレイメニューの階層化に使用されるカテゴリ
///
/// 多くの加工モードを整理するために、関連するモードをグループ化する
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, EnumIter, EnumMessage, IntoStaticStr)]
pub enum RefineCategory {
    /// 通常の単独メニュー
    #[strum(message = "")]
    Normal,
    /// URL操作サブメニュー内
    #[strum(message = "URL操作")]
    UrlActions,
    /// パス操作サブメニュー内
    #[strum(message = "パス操作")]
    Path,
    /// 行操作サブメニュー内
    #[strum(message = "行操作")]
    LineActions,
    /// トリムサブメニュー内
    #[strum(message = "トリム")]
    Trim,
    /// エスケープサブメニュー内
    #[strum(message = "エスケープ")]
    Escape,
    /// JSON整形サブメニュー内
    #[strum(message = "JSON整形")]
    JsonFormat,
    /// JSON to YAMLサブメニュー内
    #[strum(message = "YAMLへ変換")]
    ToYaml,
    /// YAML to JSONサブメニュー内
    #[strum(message = "JSONへ変換")]
    ToJson,
    /// Markdown関連サブメニュー内
    #[strum(message = "マークダウン")]
    Markdown,
    /// 日時変換サブメニュー内
    #[strum(message = "日時変換")]
    Datetime,
    /// 数値変換サブメニュー内
    #[strum(message = "数値変換")]
    Number,
}

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
    pub const SUBMENU_ORDER: [Self; 11] = [
        Self::LineActions,
        Self::UrlActions,
        Self::Path,
        Self::Markdown,
        Self::Trim,
        Self::Escape,
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
    use super::*;

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
        assert_eq!(variants.len(), 32);
    }
}
