use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumMessage, IntoStaticStr};

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
