use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumMessage, EnumProperty, EnumString, IntoStaticStr};

// ======================================================================
// 加工モード定義
// ======================================================================
/// クリップボードのテキストを加工する各モードの定義
///
/// 各バリアントは特定のテキスト加工処理(エンコード、デコード、整形、変換など)に対応している
/// `strum` マクロを使用して UI 表示用のラベルを保持している
/// カテゴリへの所属は各バリアントの `#[strum(props(category = "..."))]` で定義する
#[derive(
    Copy,
    Clone,
    Debug,
    ValueEnum,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    EnumMessage,
    EnumProperty,
    IntoStaticStr,
)]
pub enum RefineMode {
    /// URLエンコードを行う
    #[value(help = "URLエンコード")]
    #[strum(message = "URLエンコード", props(category = "UrlActions"))]
    UrlEncode,
    /// URLデコードを行う。失敗した場合は元のテキストを維持する
    #[value(help = "URLデコード")]
    #[strum(message = "URLデコード", props(category = "UrlActions"))]
    UrlDecode,
    /// URLから utm_ で始まる計測用パラメータを削除する
    #[value(help = "UTMパラメータを削除")]
    #[strum(message = "UTM除去", props(category = "UrlActions"))]
    RemoveUtm,
    /// パスからベースネームを抽出する
    #[value(help = "パスからベースネームを抽出")]
    #[strum(message = "ベースネーム抽出", props(category = "Path"))]
    ExtractBasename,
    /// パスからベースネームを抽出しダブルクォーテーションで囲む
    #[value(help = "パスからベースネームを抽出(引用符付き)")]
    #[strum(message = "ベースネーム抽出(引用符付)", props(category = "Path"))]
    ExtractBasenameQuoted,
    /// パスの前後にダブルクォーテーションを付与する
    #[value(help = "パスに引用符を付与")]
    #[strum(message = "引用符を付与", props(category = "Path"))]
    AddPathQuotes,
    /// パスの前後にあるダブルクォーテーションを削除する
    #[value(help = "パスの引用符を削除")]
    #[strum(message = "引用符を削除", props(category = "Path"))]
    RemovePathQuotes,
    /// パスのバックスラッシュをスラッシュに変換する
    #[value(help = "パスをスラッシュ区切りに変換")]
    #[strum(message = "スラッシュ区切りに変換", props(category = "Path"))]
    PathToSlash,
    /// パスのスラッシュをバックスラッシュに変換する
    #[value(help = "パスをバックスラッシュ区切りに変換")]
    #[strum(message = "バックスラッシュ区切りに変換", props(category = "Path"))]
    PathToBackslash,
    /// 行単位で昇順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "昇順で並び替え")]
    #[strum(message = "昇順で並び替え", props(category = "LineActions"))]
    SortLinesAsc,
    /// 行単位で降順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "降順で並び替え")]
    #[strum(message = "降順で並び替え", props(category = "LineActions"))]
    SortLinesDesc,
    /// 空行を削除する
    #[value(help = "空行を削除")]
    #[strum(message = "空行削除", props(category = "LineActions"))]
    RemoveEmptyLines,
    /// 重複行を削除する
    #[value(help = "重複行を削除")]
    #[strum(message = "重複行削除", props(category = "LineActions"))]
    RemoveDuplicateLines,
    /// テキスト全体の前後にある空白および改行を削除する
    #[value(help = "改行や空白を整形")]
    #[strum(message = "全体をトリム", props(category = "Trim"))]
    Trim,
    /// 行ごとに前後の空白を削除する
    #[value(help = "行単位で改行や空白を整形")]
    #[strum(message = "行単位でトリム", props(category = "Trim"))]
    TrimLines,
    /// 文字列をバックスラッシュでエスケープする
    #[value(help = "文字列をエスケープ")]
    #[strum(message = "エスケープ", props(category = "Escape"))]
    Escape,
    /// 文字列のエスケープを解除する
    #[value(help = "文字列のアンエスケープ")]
    #[strum(message = "アンエスケープ", props(category = "Escape"))]
    Unescape,
    /// 正規表現のメタ文字をエスケープする
    #[value(help = "正規表現のエスケープ")]
    #[strum(message = "正規表現エスケープ", props(category = "Escape"))]
    RegexEscape,
    /// 正規表現のエスケープを解除する
    #[value(help = "正規表現のアンエスケープ")]
    #[strum(message = "正規表現アンエスケープ", props(category = "Escape"))]
    RegexUnescape,
    /// 正規表現に一致する部分を置換文字列へ変換する (`config.toml` の `regex` 設定を使用)
    #[value(help = "正規表現で置換")]
    #[strum(message = "正規表現置換", props(category = "Regex"))]
    RegexReplace,
    /// 正規表現に一致する部分を行単位で抽出する
    #[value(help = "正規表現で抽出")]
    #[strum(message = "正規表現抽出", props(category = "Regex"))]
    RegexExtract,
    /// 正規表現に一致する部分を削除する
    #[value(help = "正規表現で削除")]
    #[strum(message = "正規表現削除", props(category = "Regex"))]
    RegexDelete,
    /// 正規表現で分割し改行で結合する
    #[value(help = "正規表現で分割")]
    #[strum(message = "正規表現分割", props(category = "Regex"))]
    RegexSplit,
    /// JSON形式をインデント整形する(キーの順序はパース時に不定となる)
    #[value(help = "JSON形式を整形(キー順序不同)")]
    #[strum(message = "JSON整形(キー順序不同)", props(category = "JsonFormat"))]
    JsonFormat,
    /// JSON形式をインデント整形する(元のキー順序を維持する)
    #[value(help = "JSON形式を整形(キー順序保持)")]
    #[strum(message = "JSON整形(キー順序保持)", props(category = "JsonFormat"))]
    JsonFormatPreserveOrder,
    /// YAML形式をJSON形式へ変換する
    #[value(help = "YAML形式をJSON形式へ変換(キー順序不同)")]
    #[strum(message = "YAML→JSON(キー順序不同)", props(category = "ToJson"))]
    YamlToJson,
    /// YAML形式をJSON形式へ変換する(元のキー順序を維持する)
    #[value(help = "YAML形式をJSON形式へ変換(キー順序保持)")]
    #[strum(message = "YAML→JSON(キー順序保持)", props(category = "ToJson"))]
    YamlToJsonPreserveOrder,
    /// JSON形式をYAML形式へ変換する
    #[value(help = "JSON形式をYAML形式へ変換(キー順序不同)")]
    #[strum(message = "JSON→YAML(キー順序不同)", props(category = "ToYaml"))]
    JsonToYaml,
    /// JSON形式をYAML形式へ変換する(元のキー順序を維持する)
    #[value(help = "JSON形式をYAML形式へ変換(キー順序保持)")]
    #[strum(message = "JSON→YAML(キー順序保持)", props(category = "ToYaml"))]
    JsonToYamlPreserveOrder,
    /// `Markdown形式のテキストをHTML形式へ変換する`
    #[value(help = "MarkdownをHTML形式へ変換")]
    #[strum(message = "Markdown→HTML", props(category = "Markdown"))]
    MarkdownToHtml,
    /// `ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する`
    #[value(help = "Excel(TSV)をMarkdown形式へ変換")]
    #[strum(message = "Excel→Markdown", props(category = "Excel"))]
    ExcelToMarkdown,
    /// `Markdown形式の表をExcel(TSV)形式へ変換する`
    #[value(help = "Markdown表をExcel(TSV)形式へ変換")]
    #[strum(message = "Markdown→Excel", props(category = "Excel"))]
    MarkdownToExcel,
    /// Excelでコピーしたセルの描画ビットマップをクリップボードの画像として保存する
    #[value(help = "Excelの見た目を画像としてクリップボードへ保存")]
    #[strum(message = "Excel→画像", props(category = "Excel"))]
    ExcelToImage,
    /// Unixタイムスタンプを日時文字列へ変換する
    #[value(help = "Unixタイムスタンプを日時文字列へ変換")]
    #[strum(
        message = "Unixタイムスタンプ→日時文字列",
        props(category = "Datetime")
    )]
    TimestampToDatetime,
    /// 日時文字列をUnixタイムスタンプへ変換する
    #[value(help = "日時文字列をUnixタイムスタンプへ変換")]
    #[strum(
        message = "日時文字列→Unixタイムスタンプ",
        props(category = "Datetime")
    )]
    DatetimeToTimestamp,
    /// 数値に対して3桁ごとのカンマを付与する(例: 1000 -> 1,000)
    #[value(help = "カンマ無し数値をカンマ区切りの数値に")]
    #[strum(message = "カンマ追加", props(category = "Number"))]
    AddComma,
    /// 数値からカンマを削除する(例: 1,000 -> 1000)
    #[value(help = "カンマ区切りの数値をカンマ無し数値に")]
    #[strum(message = "カンマ除去", props(category = "Number"))]
    RemoveComma,
    /// 識別子を `camelCase` へ変換する
    #[value(help = "識別子をcamelCaseへ変換")]
    #[strum(message = "camelCaseへ変換", props(category = "Case"))]
    ToCamelCase,
    /// 識別子を `snake_case` へ変換する
    #[value(help = "識別子をsnake_caseへ変換")]
    #[strum(message = "snake_caseへ変換", props(category = "Case"))]
    ToSnakeCase,
    /// 識別子を `PascalCase` へ変換する
    #[value(help = "識別子をPascalCaseへ変換")]
    #[strum(message = "PascalCaseへ変換", props(category = "Case"))]
    ToPascalCase,
    /// 識別子を `kebab-case` へ変換する
    #[value(help = "識別子をkebab-caseへ変換")]
    #[strum(message = "kebab-caseへ変換", props(category = "Case"))]
    ToKebabCase,
    /// 識別子を `SCREAMING_SNAKE_CASE` へ変換する
    #[value(help = "識別子をSCREAMING_SNAKE_CASEへ変換")]
    #[strum(message = "SCREAMING_SNAKE_CASEへ変換", props(category = "Case"))]
    ToScreamingSnakeCase,
}

// ======================================================================
// カテゴリ定義
// ======================================================================
/// トレイメニューの階層化に使用されるカテゴリ
///
/// 多くの加工モードを整理するために、関連するモードをグループ化する
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
    /// 正規表現操作サブメニュー内
    #[strum(message = "正規表現")]
    Regex,
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
    #[strum(message = "Markdown")]
    Markdown,
    /// Excel関連サブメニュー内
    #[strum(message = "Excel")]
    Excel,
    /// 日時変換サブメニュー内
    #[strum(message = "日時変換")]
    Datetime,
    /// 数値変換サブメニュー内
    #[strum(message = "数値変換")]
    Number,
    /// ケース変換サブメニュー内
    #[strum(message = "ケース変換")]
    Case,
}
