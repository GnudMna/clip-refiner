//! クリップボード加工モードの定義と、各モードへのディスパッチを提供するモジュール
//!
//! `RefineMode` による加工処理の統合と、クリップボードへの読み書きを担当する

pub mod datetime;
pub mod escape;
pub mod json;
pub mod line_actions;
pub mod markdown;
pub mod number;
pub mod path;
pub mod trim;
pub mod url;
pub mod utils;
pub mod yaml;

use std::borrow::Cow;

use arboard::Clipboard;
use clap::ValueEnum;
use indexmap::IndexMap;
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
    /// Markdown形式のテキストをHTML形式へ変換する
    #[value(help = "MarkdownをHTML形式へ変換")]
    #[strum(message = "Markdown→HTML")]
    MarkdownToHtml,
    /// ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する
    #[value(help = "Excel(TSV)をMarkdown形式へ変換")]
    #[strum(message = "Excel→Markdown")]
    ExcelToMarkdown,
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
    pub fn label(&self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// トレイメニューのサブメニュー表示順(`Normal` を除く)
    pub const SUBMENU_ORDER: [Self; 10] = [
        Self::LineActions,
        Self::UrlActions,
        Self::Path,
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
    pub fn label(&self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// 所属するカテゴリを取得する。トレイメニューの階層構築に利用される
    ///
    /// # Returns
    /// * `RefineCategory` - モードが属するカテゴリ
    pub fn category(&self) -> RefineCategory {
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
            Self::MarkdownToHtml | Self::ExcelToMarkdown => C::Normal,
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
// 順序保持値型
// ======================================================================
/// JSONやYAMLのパース時にキーの順序を保持するための値構造
///
/// `serde_json::Value` と似ているが、オブジェクトの保持に `IndexMap` を使用し、
/// データの順序を維持したままシリアライズ・デシリアライズが可能
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrderedValue {
    /// JSON の null 値
    Null,
    /// 真偽値
    Bool(bool),
    /// 数値
    Number(serde_json::Number),
    /// 文字列
    String(String),
    /// 配列
    Array(Vec<OrderedValue>),
    /// キー順序を保持するオブジェクト
    Object(IndexMap<String, OrderedValue>),
}

// ======================================================================
// 加工インターフェース
// ======================================================================
/// クリップボードのテキストを加工するための共通インターフェース
pub trait Refiner {
    /// テキストを加工する
    ///
    /// # Arguments
    /// * `text` - 加工前のテキスト
    ///
    /// # Returns
    /// * `Cow<'a, str>` - 加工後のテキスト(変更がない場合は元のテキストを借用)
    fn refine<'a>(&self, text: &'a str) -> Cow<'a, str>;
}

impl Refiner for RefineMode {
    fn refine<'a>(&self, text: &'a str) -> Cow<'a, str> {
        match self {
            RefineMode::UrlEncode => url::url_encode(text),
            RefineMode::UrlDecode => url::url_decode(text)
                .map(Cow::Owned)
                .unwrap_or_else(|_| Cow::Borrowed(text)),
            RefineMode::RemoveUtm => url::remove_utm_params(text),
            RefineMode::ExtractBasename => path::extract_basename(text),
            RefineMode::ExtractBasenameQuoted => path::extract_basename_quoted(text),
            RefineMode::AddPathQuotes => path::add_path_quotes(text),
            RefineMode::RemovePathQuotes => path::remove_path_quotes(text),
            RefineMode::PathToSlash => path::convert_to_forward_slash(text),
            RefineMode::PathToBackslash => path::convert_to_backslash(text),
            RefineMode::SortLinesAsc => line_actions::sort_lines(text, false),
            RefineMode::SortLinesDesc => line_actions::sort_lines(text, true),
            RefineMode::RemoveEmptyLines => line_actions::remove_empty_lines(text),
            RefineMode::RemoveDuplicateLines => line_actions::remove_duplicate_lines(text),
            RefineMode::Trim => trim::trim_text(text),
            RefineMode::TrimLines => trim::trim_lines(text),
            RefineMode::Escape => escape::escape_string(text),
            RefineMode::Unescape => escape::unescape_string(text),
            RefineMode::RegexEscape => escape::regex_escape(text),
            RefineMode::RegexUnescape => escape::regex_unescape(text),
            RefineMode::JsonFormat => json::format_json(text),
            RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(text),
            RefineMode::YamlToJson => yaml::yaml_to_json(text),
            RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(text),
            RefineMode::JsonToYaml => json::json_to_yaml(text),
            RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(text),
            RefineMode::MarkdownToHtml => markdown::markdown_to_html(text),
            RefineMode::ExcelToMarkdown => markdown::excel_to_markdown_table(text),
            RefineMode::TimestampToDatetime => datetime::timestamp_to_datetime_string(text),
            RefineMode::DatetimeToTimestamp => datetime::datetime_string_to_timestamp(text),
            RefineMode::AddComma => number::add_commas(text),
            RefineMode::RemoveComma => number::remove_commas(text),
        }
    }
}

// ======================================================================
// クリップボード処理
// ======================================================================
/// クリップボード加工の成功結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardProcessOutcome {
    /// 加工してクリップボードへ書き戻した
    Processed(String),
    /// テキストに変更がなかった
    Unchanged,
}

/// クリップボード加工の失敗理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardProcessError {
    /// クリップボードが空、またはテキスト形式ではない
    NoText,
    /// クリップボードの読み取りに失敗
    ReadFailed(String),
    /// クリップボードへの書き込みに失敗
    WriteFailed(String),
}

impl ClipboardProcessError {
    /// ユーザー向けのエラーメッセージを返す
    pub fn user_message(&self) -> &str {
        match self {
            Self::NoText => "クリップボードにテキストがありません",
            Self::ReadFailed(_) => "クリップボードの読み取りに失敗しました",
            Self::WriteFailed(_) => "クリップボードへの書き込みに失敗しました",
        }
    }
}

/// クリップボードのテキストを取得し、指定されたモードで加工して書き戻す
///
/// テキストが変更された場合のみクリップボードを更新する
///
/// # Arguments
/// * `clipboard` - `arboard::Clipboard` のミュータブルなインスタンス
/// * `mode` - 適用する加工モード (`RefineMode`)
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
pub fn process_clipboard(
    clipboard: &mut Clipboard,
    mode: RefineMode,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    let text = clipboard
        .get_text()
        .map_err(|e| ClipboardProcessError::ReadFailed(e.to_string()))?;

    if text.is_empty() {
        return Err(ClipboardProcessError::NoText);
    }

    let refined = mode.refine(&text);

    if refined == text {
        return Ok(ClipboardProcessOutcome::Unchanged);
    }

    let result = refined.into_owned();
    clipboard
        .set_text(result.clone())
        .map_err(|e| ClipboardProcessError::WriteFailed(e.to_string()))?;
    Ok(ClipboardProcessOutcome::Processed(result))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use arboard::Clipboard;

    /// RefineMode のラベルとカテゴリが期待どおりであること
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
            RefineCategory::Normal
        );
    }

    /// 全モードのカテゴリが SUBMENU_ORDER で網羅されていること
    #[test]
    fn test_submenu_order_covers_all_categories() {
        use std::collections::HashSet;

        let used: HashSet<_> = RefineMode::iter()
            .map(|m| m.category())
            .filter(|c| *c != RefineCategory::Normal)
            .collect();
        let ordered: HashSet<_> = RefineCategory::SUBMENU_ORDER.into_iter().collect();

        assert_eq!(
            used, ordered,
            "RefineCategory::SUBMENU_ORDER が全サブメニューカテゴリを網羅していません"
        );
    }

    /// 全 RefineMode の label が空でないこと
    #[test]
    fn test_all_refine_modes_have_nonempty_labels() {
        for mode in RefineMode::iter() {
            assert!(!mode.label().is_empty(), "{mode:?} の label が空です");
        }
    }

    /// is_deferred_in_menu が Datetime / Number のみ true であること
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

    /// to_json_list が全モード分の有効な JSON を返すこと
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

    /// selector_modes が全モードをトレイメニュー相当の順序で返すこと
    #[test]
    fn test_selector_modes_order() {
        let ordered = RefineMode::selector_modes();
        assert_eq!(ordered.len(), RefineMode::iter().count());

        let normal_count = RefineMode::iter()
            .filter(|m| m.category() == RefineCategory::Normal)
            .count();
        assert!(normal_count > 0);
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

    /// RefineMode のバリアント数と主要バリアントの存在を確認すること
    #[test]
    fn test_refine_mode_variants() {
        let variants: Vec<_> = RefineMode::iter().collect();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLinesAsc));
        assert!(variants.contains(&RefineMode::SortLinesDesc));
        assert!(variants.contains(&RefineMode::TimestampToDatetime));
        assert_eq!(variants.len(), 31);
    }

    /// `ClipboardProcessError` がユーザー向けメッセージを返すこと
    #[test]
    fn test_clipboard_process_error_user_message() {
        assert_eq!(
            ClipboardProcessError::NoText.user_message(),
            "クリップボードにテキストがありません"
        );
        assert_eq!(
            ClipboardProcessError::ReadFailed("detail".to_string()).user_message(),
            "クリップボードの読み取りに失敗しました"
        );
        assert_eq!(
            ClipboardProcessError::WriteFailed("detail".to_string()).user_message(),
            "クリップボードへの書き込みに失敗しました"
        );
    }

    /// クリップボード処理の統合テスト
    ///
    /// システムクリップボードへのアクセスが必要なため、通常の `cargo test` では除外される
    /// 手動実行: `cargo test test_process_clipboard_integration -- --ignored`
    #[test]
    #[ignore = "システムクリップボードへのアクセスが必要"]
    fn test_process_clipboard_integration() {
        let mut cb = Clipboard::new().expect("クリップボードの初期化に失敗");

        let unique_str_1 = "  clip_refiner_test_1  ";
        cb.set_text(unique_str_1.to_string())
            .expect("クリップボードへの書き込みに失敗");
        assert_eq!(
            cb.get_text().expect("クリップボードの読み取りに失敗"),
            unique_str_1
        );
        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Processed(
                "clip_refiner_test_1".to_string()
            ))
        );

        let unique_str_2 = "clip_refiner_test_2";
        cb.set_text(unique_str_2.to_string())
            .expect("クリップボードへの書き込みに失敗");
        assert_eq!(
            cb.get_text().expect("クリップボードの読み取りに失敗"),
            unique_str_2
        );
        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
    }

    /// 全てのRefineModeバリアントを網羅するテーブル駆動テスト
    /// 各モードが正しく配線され、期待通りの加工を行うかを確認する
    #[test]
    fn test_all_refine_modes() {
        const CASES: &[(RefineMode, &str, &str)] = &[
            (
                RefineMode::UrlEncode,
                "あいう",
                "%E3%81%82%E3%81%84%E3%81%86",
            ),
            (
                RefineMode::UrlDecode,
                "%E3%81%82%E3%81%84%E3%81%86",
                "あいう",
            ),
            (
                RefineMode::RemoveUtm,
                "http://example.com/?utm_source=test",
                "http://example.com/",
            ),
            (
                RefineMode::ExtractBasename,
                "C:\\path\\to\\file.txt",
                "file.txt",
            ),
            (
                RefineMode::ExtractBasenameQuoted,
                "C:\\path\\to\\file.txt",
                "\"file.txt\"",
            ),
            (
                RefineMode::AddPathQuotes,
                "C:\\path\\to\\file.txt",
                "\"C:\\path\\to\\file.txt\"",
            ),
            (
                RefineMode::RemovePathQuotes,
                "\"C:\\path\\to\\file.txt\"",
                "C:\\path\\to\\file.txt",
            ),
            (
                RefineMode::PathToSlash,
                "C:\\path\\to\\file.txt",
                "C:/path/to/file.txt",
            ),
            (
                RefineMode::PathToBackslash,
                "C:/path/to/file.txt",
                "C:\\path\\to\\file.txt",
            ),
            (RefineMode::SortLinesAsc, "c\na\nb", "a\nb\nc"),
            (RefineMode::SortLinesDesc, "a\nc\nb", "c\nb\na"),
            (RefineMode::RemoveEmptyLines, "a\n\nb", "a\nb"),
            (RefineMode::RemoveDuplicateLines, "a\na\nb", "a\nb"),
            (RefineMode::Trim, "  abc  ", "abc"),
            (RefineMode::TrimLines, " a \n b ", "a\nb"),
            (RefineMode::Escape, "\"", "\\\""),
            (RefineMode::Unescape, "\\\"", "\""),
            (RefineMode::RegexEscape, "(.*)", "\\(\\.\\*\\)"),
            (RefineMode::RegexUnescape, "\\(\\.\\*\\)", "(.*)"),
            (
                RefineMode::JsonFormat,
                "{\"b\":1,\"a\":2}",
                "{\n  \"a\": 2,\n  \"b\": 1\n}",
            ),
            (
                RefineMode::JsonFormatPreserveOrder,
                "{\"b\":1,\"a\":2}",
                "{\n  \"b\": 1,\n  \"a\": 2\n}",
            ),
            (
                RefineMode::YamlToJson,
                "a: 1\nb: 2",
                "{\n  \"a\": 1,\n  \"b\": 2\n}",
            ),
            (
                RefineMode::YamlToJsonPreserveOrder,
                "a: 1\nb: 2",
                "{\n  \"a\": 1,\n  \"b\": 2\n}",
            ),
            (RefineMode::JsonToYaml, "{\"a\":1}", "a: 1\n"),
            (RefineMode::JsonToYamlPreserveOrder, "{\"a\":1}", "a: 1\n"),
            (
                RefineMode::MarkdownToHtml,
                "**bold**",
                "<p><strong>bold</strong></p>",
            ),
            (
                RefineMode::ExcelToMarkdown,
                "A\tB\n1\t2",
                "| A | B |\n|---|---|\n| 1 | 2 |",
            ),
            (RefineMode::AddComma, "1000", "1,000"),
            (RefineMode::RemoveComma, "1,000", "1000"),
        ];

        assert_eq!(
            CASES.len() + 2,
            RefineMode::iter().count(),
            "固定ケースと日時モード2件の合計が RefineMode バリアント数と一致しません"
        );

        for mode in RefineMode::iter() {
            match mode {
                RefineMode::TimestampToDatetime => {
                    let input = "1672531200";
                    let actual = mode.refine(input);
                    let expected = datetime::timestamp_to_datetime_string(input);
                    assert_eq!(actual, expected);
                    assert_ne!(actual.as_ref(), input);
                }
                RefineMode::DatetimeToTimestamp => {
                    let datetime_input = datetime::timestamp_to_datetime_string("1672531200");
                    let actual = mode.refine(&datetime_input);
                    assert_eq!(actual, "1672531200");
                }
                other => {
                    let (input, expected) = CASES
                        .iter()
                        .find(|(m, _, _)| *m == other)
                        .map(|(_, input, expected)| (*input, *expected))
                        .unwrap_or_else(|| panic!("TestCase missing for {:?}", other));
                    let actual = other.refine(input);
                    assert_eq!(
                        actual, expected,
                        "Failed at mode: {:?}\nInput: {}\nExpected: {}\nActual: {}",
                        other, input, expected, actual
                    );
                }
            }
        }
    }
}
