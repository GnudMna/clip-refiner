// ======================================================================
// 各項目の説明コメント (テンプレート生成と新規キー挿入で共有)
// ======================================================================
pub(crate) const DOC_VERSION: &str = "設定スキーマのバージョン";
pub(crate) const DOC_MODE: &str = "使用する加工モード (`pipeline` が空の場合に監視で適用)";
pub(crate) const DOC_PIPELINE: &str = "監視時に順に適用する加工モードの連鎖 (PascalCase の配列。空の場合は `mode` のみ。例: [\"UrlDecode\", \"Trim\"])";
pub(crate) const DOC_FAVORITE_MODES: &str =
    "お気に入り変換モード (PascalCase の配列。例: [\"UrlDecode\", \"Trim\"])";
pub(crate) const DOC_INTERVAL_MS: &str = "クリップボードのポーリング間隔 (ミリ秒、100〜60000)";
pub(crate) const DOC_MONITOR_MODE: &str = "監視方式 (\"Polling\" または \"Event\")";
pub(crate) const DOC_IS_PAUSED: &str = "監視を一時停止するかどうか";
pub(crate) const DOC_HISTORY_ENABLED: &str = "加工履歴の有効・無効";
pub(crate) const DOC_HISTORY_LIMIT: &str = "履歴の最大保持件数 (1〜100)";
pub(crate) const DOC_NS_ENABLED: &str = "デスクトップ通知の有効・無効";
pub(crate) const DOC_NS_NOTIFY_MODE: &str = "モード変更時の通知";
pub(crate) const DOC_NS_NOTIFY_RESULT: &str = "通知にクリップボードの内容を表示するかどうか";
pub(crate) const DOC_NS_NOTIFY_PAUSE: &str = "一時停止切替時の通知";
pub(crate) const DOC_HOTKEY_SELECTOR: &str = "クイックセレクタ表示";
pub(crate) const DOC_HOTKEY_NOTIFICATION: &str = "成功通知の ON/OFF";
pub(crate) const DOC_HOTKEY_PAUSE: &str = "監視の一時停止・再開";
pub(crate) const DOC_HOTKEY_UNDO: &str = "直近の加工を取り消し";
pub(crate) const DOC_HOTKEY_TEXT_SELECTOR: &str = "登録文字列セレクター表示";
pub(crate) const DOC_HOTKEY_OCR: &str = "画面範囲選択 OCR";
pub(crate) const DOC_HOTKEY_QUIT: &str = "アプリケーション終了";
pub(crate) const DOC_HOTKEY_FAVORITE_SLOTS: &str = "お気に入り変換モード用ホットキー (登録順インデックスに対応。未指定スロットは Alt+Shift+1〜9 / Alt+Shift+F1〜F11。空文字で無効)";
pub(crate) const DOC_REGEX_PATTERN: &str = "正規表現パターン";
pub(crate) const DOC_REGEX_REPLACEMENT: &str =
    "置換文字列 (RegexReplace で使用。キャプチャグループは $1 形式)";
pub(crate) const DOC_REGEX_CASE_INSENSITIVE: &str = "大文字小文字を無視する ((?i) 相当)";
pub(crate) const DOC_REGEX_MULTILINE: &str = "複数行モード ((?m) 相当)";

pub(crate) const SECTION_BASIC: &str = "基本";
pub(crate) const SECTION_MONITOR: &str = "監視";
pub(crate) const SECTION_HISTORY: &str = "履歴";
pub(crate) const SECTION_NOTIFICATION: &str = "通知";
pub(crate) const SECTION_HOTKEYS: &str =
    "グローバルホットキー (\"Alt+Shift+S\" 形式。変更後は自動または「設定を再読み込み」で反映)";
pub(crate) const SECTION_REGEX: &str = "正規表現加工モード用のパターンと置換文字列";
pub(crate) const SECTION_TEXTS: &str = "クリップボードへコピーする登録文字列 (`[[texts]]` 形式)";
pub(crate) const DOC_TEXT_LABEL: &str = "一覧表示用のラベル";
pub(crate) const DOC_TEXT_BODY: &str = "クリップボードへコピーする本文";

pub(crate) const SECTION_RULE: &str =
    "# -----------------------------------------------------------------------------";
pub(crate) const TABLE_INDENT: &str = "  ";
