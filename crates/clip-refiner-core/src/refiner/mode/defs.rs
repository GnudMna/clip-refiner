use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumMessage, EnumString, IntoStaticStr};

// ======================================================================
// 変換モード定義
// ======================================================================
/// クリップボードのテキストを加工する各モードの定義
///
/// 各バリアントは特定のテキスト加工処理(エンコード、デコード、整形、変換など)に対応している
/// `strum` マクロを使用して UI 表示用のラベルを保持している
/// カテゴリへの所属は `RefineMode::category` の網羅的 `match` で定義する
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    EnumMessage,
    EnumString,
    IntoStaticStr,
    Display,
)]
#[strum(serialize_all = "kebab-case")]
pub enum RefineMode {
    /// URLエンコードを行う
    #[strum(message = "URLエンコード")]
    UrlEncode,
    /// URLデコードを行う。失敗した場合は元のテキストを維持する
    #[strum(message = "URLデコード")]
    UrlDecode,
    /// URLから utm_ で始まる計測用パラメータを削除する
    #[strum(message = "UTM除去")]
    RemoveUtm,
    /// パスからベースネームを抽出する
    #[strum(message = "ベースネーム抽出")]
    ExtractBasename,
    /// パスからベースネームを抽出しダブルクォーテーションで囲む
    #[strum(message = "ベースネーム抽出(引用符付)")]
    ExtractBasenameQuoted,
    /// パスの前後にダブルクォーテーションを付与する
    #[strum(message = "引用符を付与")]
    AddPathQuotes,
    /// パスの前後にあるダブルクォーテーションを削除する
    #[strum(message = "引用符を削除")]
    RemovePathQuotes,
    /// パスのバックスラッシュをスラッシュに変換する
    #[strum(message = "スラッシュ区切りに変換")]
    PathToSlash,
    /// パスのスラッシュをバックスラッシュに変換する
    #[strum(message = "バックスラッシュ区切りに変換")]
    PathToBackslash,
    /// 行単位で昇順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[strum(message = "昇順で並び替え")]
    SortLinesAsc,
    /// 行単位で降順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[strum(message = "降順で並び替え")]
    SortLinesDesc,
    /// 空行を削除する
    #[strum(message = "空行削除")]
    RemoveEmptyLines,
    /// 重複行を削除する
    #[strum(message = "重複行削除")]
    RemoveDuplicateLines,
    /// テキスト全体の前後にある空白および改行を削除する
    #[strum(message = "全体をトリム")]
    Trim,
    /// 行ごとに前後の空白を削除する
    #[strum(message = "行単位でトリム")]
    TrimLines,
    /// 文字列をバックスラッシュでエスケープする
    #[strum(message = "エスケープ")]
    Escape,
    /// 文字列のエスケープを解除する
    #[strum(message = "アンエスケープ")]
    Unescape,
    /// 正規表現のメタ文字をエスケープする
    #[strum(message = "正規表現エスケープ")]
    RegexEscape,
    /// 正規表現のエスケープを解除する
    #[strum(message = "正規表現アンエスケープ")]
    RegexUnescape,
    /// 正規表現に一致する部分を置換文字列へ変換する (`config.toml` の `regex` 設定を使用)
    #[strum(message = "正規表現置換")]
    RegexReplace,
    /// 正規表現に一致する部分を行単位で抽出する
    #[strum(message = "正規表現抽出")]
    RegexExtract,
    /// 正規表現に一致する部分を削除する
    #[strum(message = "正規表現削除")]
    RegexDelete,
    /// 正規表現で分割し改行で結合する
    #[strum(message = "正規表現分割")]
    RegexSplit,
    /// JSON形式をインデント整形する(キーを辞書順に並べ替える)
    #[strum(message = "JSON整形(キー順序ソート)")]
    JsonFormat,
    /// JSON形式をインデント整形する(元のキー順序を維持する)
    #[strum(message = "JSON整形(キー順序保持)")]
    JsonFormatPreserveOrder,
    /// YAML形式をJSON形式へ変換する
    #[strum(message = "YAML→JSON(キー順序不同)")]
    YamlToJson,
    /// YAML形式をJSON形式へ変換する(元のキー順序を維持する)
    #[strum(message = "YAML→JSON(キー順序保持)")]
    YamlToJsonPreserveOrder,
    /// JSON形式をYAML形式へ変換する(キーを辞書順に並べ替える)
    #[strum(message = "JSON→YAML(キー順序ソート)")]
    JsonToYaml,
    /// JSON形式をYAML形式へ変換する(元のキー順序を維持する)
    #[strum(message = "JSON→YAML(キー順序保持)")]
    JsonToYamlPreserveOrder,
    /// `Markdown形式のテキストをHTML形式へ変換する`
    #[strum(message = "Markdown→HTML")]
    MarkdownToHtml,
    /// Markdown形式のテキストをリッチテキスト (`CF_HTML`) としてクリップボードへ書き込む
    ///
    /// プレーンテキスト側には変換前の Markdown を残す。テキストのみの加工では HTML ソースを返す
    #[strum(message = "Markdown→リッチテキスト")]
    MarkdownToRichText,
    /// HTML を Markdown 形式へ変換する
    ///
    /// クリップボードにHTML形式 (`CF_HTML`) があればテキストより優先して入力にする
    #[strum(message = "HTML→Markdown")]
    HtmlToMarkdown,
    /// `ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する`
    #[strum(message = "Excel→Markdown")]
    ExcelToMarkdown,
    /// `Markdown形式の表をExcel(TSV)形式へ変換する`
    #[strum(message = "Markdown→Excel")]
    MarkdownToExcel,
    /// Excelでコピーしたセルの描画ビットマップをクリップボードの画像として保存する
    #[strum(message = "Excel→画像")]
    ExcelToImage,
    /// Unixタイムスタンプを日時文字列へ変換する
    #[strum(message = "Unixタイムスタンプ→日時文字列")]
    TimestampToDatetime,
    /// 日時文字列をUnixタイムスタンプへ変換する
    #[strum(message = "日時文字列→Unixタイムスタンプ")]
    DatetimeToTimestamp,
    /// 数値に対して3桁ごとのカンマを付与する(例: 1000 -> 1,000)
    #[strum(message = "カンマ追加")]
    AddComma,
    /// 数値からカンマを削除する(例: 1,000 -> 1000)
    #[strum(message = "カンマ除去")]
    RemoveComma,
    /// 識別子を `camelCase` へ変換する
    #[strum(message = "camelCaseへ変換")]
    ToCamelCase,
    /// 識別子を `snake_case` へ変換する
    #[strum(message = "snake_caseへ変換")]
    ToSnakeCase,
    /// 識別子を `PascalCase` へ変換する
    #[strum(message = "PascalCaseへ変換")]
    ToPascalCase,
    /// 識別子を `kebab-case` へ変換する
    #[strum(message = "kebab-caseへ変換")]
    ToKebabCase,
    /// 識別子を `SCREAMING_SNAKE_CASE` へ変換する
    #[strum(message = "SCREAMING_SNAKE_CASEへ変換")]
    ToScreamingSnakeCase,
    /// 全角の英数字・記号・スペース・カタカナを半角へ変換する
    #[strum(message = "半角へ変換")]
    ToHalfwidth,
    /// 半角の英数字・記号・スペース・カタカナを全角へ変換する
    #[strum(message = "全角へ変換")]
    ToFullwidth,
}

// ======================================================================
// カテゴリ定義
// ======================================================================
/// トレイメニューの階層化に使用されるカテゴリ
///
/// 多くの変換モードを整理するために、関連するモードをグループ化する
#[derive(
    Debug, PartialEq, Eq, Clone, Copy, Hash, EnumIter, EnumMessage, EnumString, IntoStaticStr,
)]
pub enum RefineCategory {
    /// 通常の単独メニュー
    #[strum(message = "")]
    Normal,
    /// URL操作サブメニュー内
    #[strum(message = "URL操作")]
    UrlActions,
    /// パス操作サブメニュー内
    #[strum(message = "ファイルパス")]
    Path,
    /// 行操作サブメニュー内
    #[strum(message = "行の並べ替え・削除")]
    LineActions,
    /// トリムサブメニュー内
    #[strum(message = "空白の除去")]
    Trim,
    /// エスケープサブメニュー内
    #[strum(message = "エスケープ・解除")]
    Escape,
    /// 正規表現操作サブメニュー内
    #[strum(message = "正規表現")]
    Regex,
    /// JSON整形サブメニュー内
    #[strum(message = "JSON整形")]
    JsonFormat,
    /// JSON to YAMLサブメニュー内
    #[strum(message = "JSON→YAML")]
    ToYaml,
    /// YAML to JSONサブメニュー内
    #[strum(message = "YAML→JSON")]
    ToJson,
    /// Markdown関連サブメニュー内
    #[strum(message = "Markdown・HTML")]
    Markdown,
    /// Excel関連サブメニュー内
    #[strum(message = "Excel")]
    Excel,
    /// 日時変換サブメニュー内
    #[strum(message = "Unixタイムスタンプ")]
    Datetime,
    /// 数値変換サブメニュー内
    #[strum(message = "桁区切りカンマ")]
    Number,
    /// ケース変換サブメニュー内
    #[strum(message = "識別子の書式")]
    Case,
    /// 全角・半角サブメニュー内
    #[strum(message = "全角・半角")]
    Width,
}
