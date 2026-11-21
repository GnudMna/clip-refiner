// ======================================================================
// 各項目の説明コメント (テンプレート生成と新規キー挿入で共有)
// ======================================================================
pub(crate) const DOC_VERSION: &str = "設定スキーマのバージョン";
pub(crate) const DOC_MODE: &str = "使用する変換モード (`pipeline` が空の場合に監視で適用)";
pub(crate) const DOC_PIPELINE: &str = "監視時に順に適用する変換モードの連鎖 (PascalCase の配列。空の場合は `mode` のみ。例: [\"UrlDecode\", \"Trim\"])";
pub(crate) const DOC_FAVORITE_MODES: &str =
    "お気に入り変換モード (PascalCase の配列。例: [\"UrlDecode\", \"Trim\"])";
pub(crate) const DOC_INTERVAL_MS: &str = "クリップボードのポーリング間隔 (ミリ秒、100〜60000)";
pub(crate) const DOC_MONITOR_MODE: &str = "監視方式 (\"Polling\" または \"Event\")";
pub(crate) const DOC_SELECTOR_WEBVIEW_RETENTION: &str =
    "セレクター WebView の保持方針 (\"Keep\" / \"DisposeOnHide\" / \"DisposeAfterIdle\")";
pub(crate) const DOC_SELECTOR_WEBVIEW_IDLE_MINUTES: &str =
    "DisposeAfterIdle 時に非表示から破棄するまでの分数 (1〜1440。既定 5)";
pub(crate) const DOC_IS_PAUSED: &str = "監視を一時停止するかどうか";
pub(crate) const DOC_HISTORY_ENABLED: &str = "加工履歴の有効・無効";
pub(crate) const DOC_CONTINUOUS_RECORD_ENABLED: &str =
    "連続記録モード (監視時は加工せず元テキストを記録)";
pub(crate) const DOC_CONTINUOUS_RECORD_LIMIT: &str = "連続記録の最大保持件数 (1〜100)";
pub(crate) const DOC_HISTORY_LIMIT: &str = "履歴の最大保持件数 (1〜100)";
pub(crate) const DOC_NS_ENABLED: &str = "デスクトップ通知の有効・無効 (エラー通知は常に表示)";
pub(crate) const DOC_NS_NOTIFY_PROCESS: &str = "加工完了時の通知";
pub(crate) const DOC_NS_NOTIFY_PAUSE: &str = "一時停止切替時の通知";
pub(crate) const DOC_NS_NOTIFY_HISTORY_RESTORE: &str = "履歴復元時の通知";
pub(crate) const DOC_NS_NOTIFY_OCR: &str = "OCR 完了時の通知";
pub(crate) const DOC_NS_NOTIFY_REGISTERED_CLIP: &str = "登録クリップ操作時の通知";
pub(crate) const DOC_NS_NOTIFY_FAVORITES: &str = "お気に入り操作時の通知";
pub(crate) const DOC_NS_NOTIFY_CONTINUOUS_RECORD: &str = "連続記録操作時の通知";
pub(crate) const DOC_NS_NOTIFY_RESULT: &str = "通知にクリップボードの内容を表示するかどうか";
pub(crate) const DOC_HOTKEY_SELECTOR: &str = "クイックセレクタ表示";
pub(crate) const DOC_HOTKEY_NOTIFICATION: &str = "成功通知の ON/OFF";
pub(crate) const DOC_HOTKEY_PAUSE: &str = "監視の一時停止・再開";
pub(crate) const DOC_HOTKEY_UNDO: &str = "直近の加工を取り消し";
pub(crate) const DOC_HOTKEY_CLIP_SELECTOR: &str = "登録クリップセレクター表示";
pub(crate) const DOC_HOTKEY_OCR: &str = "画面範囲選択 OCR";
pub(crate) const DOC_HOTKEY_CONTINUOUS_RECORD: &str = "連続記録の有効・無効切替";
pub(crate) const DOC_HOTKEY_CONTINUOUS_RECORD_UNDO: &str = "連続記録の直近 1 件取り消し";
pub(crate) const DOC_HOTKEY_CONTINUOUS_RECORD_PASTE: &str =
    "連続記録キュー先頭をクリップボードへ載せて貼り付け";
pub(crate) const DOC_HOTKEY_QUIT: &str = "アプリケーション終了";
pub(crate) const DOC_HOTKEY_FAVORITE_SLOTS: &str = "お気に入り変換モード用ホットキー (登録順インデックスに対応。未指定スロットは Alt+Shift+1〜9 / Alt+Shift+F1〜F11。空文字で無効)";
pub(crate) const DOC_REGEX_ACTIVE_PRESET_ID: &str = "有効な正規表現プリセットの ID";
pub(crate) const DOC_REGEX_PRESETS: &str =
    "名前付き正規表現プリセット (steps を上から順に適用。最大 20 件 / 各最大 10 ステップ)";

pub(crate) const SECTION_BASIC: &str = "基本";
pub(crate) const SECTION_MONITOR: &str = "監視";
pub(crate) const SECTION_SELECTOR_WEBVIEW: &str = "セレクター WebView";
pub(crate) const SECTION_HISTORY: &str = "履歴";
pub(crate) const SECTION_CONTINUOUS_RECORD: &str = "連続記録";
pub(crate) const SECTION_NOTIFICATION: &str = "通知";
pub(crate) const SECTION_HOTKEYS: &str =
    "グローバルホットキー (\"Alt+Shift+S\" 形式。変更後は数秒以内に自動反映)";
pub(crate) const SECTION_REGEX: &str = "正規表現プリセット (パターン・置換文字列の名前付きセット)";

pub(crate) const SECTION_RULE: &str =
    "# -----------------------------------------------------------------------------";
pub(crate) const TABLE_INDENT: &str = "  ";
