use crate::consts;

// ======================================================================
// 各項目の説明コメント (テンプレート生成と新規キー挿入で共有)
// ======================================================================
// 件数・範囲を含む文言は `consts` から組み立て、設定リファレンス (`docs/user/configuration.md`) との同期テスト
// (`config/spec.rs`) が生成 TOML コメントのドリフトを検出する

pub(crate) const DOC_VERSION: &str = "設定スキーマのバージョン";
pub(crate) const DOC_AUTO_REFINE_ENABLED: &str =
    "コピー時に `mode` / `pipeline` で自動変換するかどうか";
pub(crate) const DOC_MODE: &str = "使用する変換モード (`pipeline` が空の場合に監視で適用)";
pub(crate) const DOC_MONITOR_MODE: &str = "監視方式 (\"Polling\" または \"Event\")";
pub(crate) const DOC_SELECTOR_WEBVIEW_RETENTION: &str =
    "セレクター WebView の保持方針 (\"Keep\" / \"DisposeOnHide\" / \"DisposeAfterIdle\")";
pub(crate) const DOC_HISTORY_ENABLED: &str = "加工履歴の有効・無効";

pub(crate) const DOC_KEEP_CLIPBOARD_EXCLUSION: &str =
    "手動加工・登録クリップのコピーで、履歴・クラウド同期から除外する形式を付ける";
pub(crate) const DOC_CONTINUOUS_RECORD_ENABLED: &str =
    "連続記録モード (監視時は加工せず元テキストを記録)";
pub(crate) const DOC_NS_ENABLED: &str = "デスクトップ通知の有効・無効 (エラー通知は常に表示)";
pub(crate) const DOC_NS_NOTIFY_PROCESS: &str = "加工完了時の通知";
pub(crate) const DOC_NS_NOTIFY_AUTO_REFINE: &str = "自動変換の切替時の通知";
pub(crate) const DOC_NS_NOTIFY_HISTORY_RESTORE: &str = "履歴復元時の通知";
pub(crate) const DOC_NS_NOTIFY_OCR: &str = "OCR 完了時の通知";
pub(crate) const DOC_NS_NOTIFY_REGISTERED_CLIP: &str = "登録クリップ操作時の通知";
pub(crate) const DOC_NS_NOTIFY_FAVORITES: &str = "お気に入り操作時の通知";
pub(crate) const DOC_NS_NOTIFY_CONTINUOUS_RECORD: &str = "連続記録モードの操作時の通知";
pub(crate) const DOC_NS_NOTIFY_RESULT: &str = "通知にクリップボードの内容を表示するかどうか";
pub(crate) const DOC_HOTKEY_SELECTOR: &str = "クイックセレクタ表示";
pub(crate) const DOC_HOTKEY_NOTIFICATION: &str = "成功通知の ON/OFF";
pub(crate) const DOC_HOTKEY_AUTO_REFINE: &str = "自動変換の有効・無効切替";
pub(crate) const DOC_HOTKEY_UNDO: &str = "直近の加工を取り消し";
pub(crate) const DOC_HOTKEY_CLIP_SELECTOR: &str = "登録クリップセレクター表示";
pub(crate) const DOC_HOTKEY_OCR: &str = "画面範囲選択 OCR";
pub(crate) const DOC_HOTKEY_CONTINUOUS_RECORD: &str = "連続記録モードの有効・無効切替";
pub(crate) const DOC_HOTKEY_CONTINUOUS_RECORD_UNDO: &str =
    "連続記録モードの直近 1 件の記録を取り消し";
pub(crate) const DOC_HOTKEY_CONTINUOUS_RECORD_PASTE: &str =
    "連続記録モードのキュー先頭をクリップボードへ載せて貼り付け";
pub(crate) const DOC_HOTKEY_QUIT: &str = "アプリケーション終了";
pub(crate) const DOC_REGEX_ACTIVE_PRESET_ID: &str = "有効な正規表現プリセットの ID";

pub(crate) const SECTION_BASIC: &str = "基本";
pub(crate) const SECTION_MONITOR: &str = "監視";
pub(crate) const SECTION_SELECTOR_WEBVIEW: &str = "セレクター WebView";
pub(crate) const SECTION_HISTORY: &str = "履歴";
pub(crate) const SECTION_CONTINUOUS_RECORD: &str = "連続記録モード";
pub(crate) const SECTION_NOTIFICATION: &str = "通知";
pub(crate) const SECTION_REGEX: &str = "正規表現プリセット (パターン・置換文字列の名前付きセット)";

pub(crate) const SECTION_RULE: &str =
    "# -----------------------------------------------------------------------------";
pub(crate) const TABLE_INDENT: &str = "  ";

/// 加工パイプラインの説明コメントを返す
pub(crate) fn doc_pipeline() -> String {
    format!(
        "監視時に順に適用する変換モードの連鎖 (PascalCase の配列、最大 {} 段。空の場合は `mode` のみ。例: [\"UrlDecode\", \"Trim\"])",
        consts::MAX_PIPELINE_LENGTH
    )
}

/// お気に入り変換モードの説明コメントを返す
pub(crate) fn doc_favorite_modes() -> String {
    format!(
        "お気に入り変換モード (PascalCase の配列、最大 {} 件。例: [\"UrlDecode\", \"Trim\"])",
        consts::MAX_FAVORITE_MODES
    )
}

/// ポーリング間隔の説明コメントを返す
pub(crate) fn doc_interval_ms() -> String {
    format!(
        "クリップボードのポーリング間隔 (ミリ秒、{}〜{})",
        consts::MIN_INTERVAL_MS,
        consts::MAX_INTERVAL_MS
    )
}

/// セレクター `WebView` アイドル分数の説明コメントを返す
pub(crate) fn doc_selector_webview_idle_minutes() -> String {
    format!(
        "DisposeAfterIdle 時に非表示から破棄するまでの分数 ({}〜{}。既定 {})",
        consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES,
        consts::MAX_SELECTOR_WEBVIEW_IDLE_MINUTES,
        consts::DEFAULT_SELECTOR_WEBVIEW_IDLE_MINUTES
    )
}

/// 連続記録件数上限の説明コメントを返す
pub(crate) fn doc_continuous_record_limit() -> String {
    format!(
        "連続記録モードの最大保持件数 ({}〜{})",
        consts::MIN_CONTINUOUS_RECORD_LIMIT,
        consts::MAX_CONTINUOUS_RECORD_LIMIT
    )
}

/// 履歴件数上限の説明コメントを返す
pub(crate) fn doc_history_limit() -> String {
    format!(
        "履歴の最大保持件数 ({}〜{})",
        consts::MIN_HISTORY_LIMIT,
        consts::MAX_HISTORY_LIMIT
    )
}

/// お気に入りスロットホットキーの説明コメントを返す
pub(crate) fn doc_hotkey_favorite_slots() -> String {
    let digit_slots = 9.min(consts::MAX_FAVORITE_MODES);
    let function_slots = consts::MAX_FAVORITE_MODES.saturating_sub(digit_slots);
    format!(
        "お気に入り変換モード用ホットキー (登録順インデックスに対応。未指定スロットは Alt+Shift+1〜{digit_slots} / Alt+Shift+F1〜F{function_slots}。空文字で無効)"
    )
}

/// 正規表現プリセット配列の説明コメントを返す
pub(crate) fn doc_regex_presets() -> String {
    format!(
        "名前付き正規表現プリセット (steps を上から順に適用。最大 {} 件 / 各最大 {} ステップ)",
        consts::MAX_REGEX_PRESETS,
        consts::MAX_REGEX_STEPS_PER_PRESET
    )
}

/// ホットキーセクション見出しを返す
pub(crate) fn section_hotkeys() -> String {
    format!(
        "グローバルホットキー (\"{}\" 形式。変更後は数秒以内に自動反映)",
        consts::DEFAULT_HOTKEY_QUICK_SELECTOR
    )
}
