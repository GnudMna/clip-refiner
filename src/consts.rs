// ======================================================================
// アプリケーション情報
// ======================================================================
/// アプリケーションの表示名 (`PascalCase`)
///
/// ウィンドウタイトルやメニューの表示に使用される
pub const APP_NAME: &str = "ClipRefiner";

/// アプリケーションの識別子 (kebab-case)
///
/// 設定フォルダ名やデータ保存パスの生成に使用される
#[cfg_attr(windows, allow(dead_code))]
pub const APP_NAME_KEBAB: &str = "clip-refiner";

// ======================================================================
// 識別子
// ======================================================================
/// アプリケーションの一意な識別子
///
/// 多重起動防止やレジストリ、設定のスコープ分離に使用される
pub const APP_ID: &str = "com.y_hirata.clip-refiner";

// ======================================================================
// 設定
// ======================================================================
/// 設定ファイルのスキーマバージョン
///
/// スキーマを変更したら 1 ずつ増やし、`config/migrate.rs` に `migrate_vN_to_vM` の実装を追加する
pub const CONFIG_VERSION: u32 = 2;

/// 履歴のデフォルト最大保持数
pub const DEFAULT_HISTORY_LIMIT: usize = 10;

/// 履歴の最小保持数
pub const MIN_HISTORY_LIMIT: usize = 1;

/// 履歴の最大保持数
pub const MAX_HISTORY_LIMIT: usize = 100;

/// ポーリング間隔の最小値(ミリ秒)
pub const MIN_INTERVAL_MS: u64 = 100;

/// ポーリング間隔の最大値(ミリ秒)
pub const MAX_INTERVAL_MS: u64 = 60_000;

/// クリップボード本文の最大バイト数 (2 MiB)
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// 正規表現パターンの最大バイト数 (8 KiB)
pub const MAX_REGEX_PATTERN_BYTES: usize = 8 * 1024;

/// JSON / YAML / Markdown パーサー入力の最大バイト数 (1 MiB)
pub const MAX_PARSER_INPUT_BYTES: usize = 1024 * 1024;

/// 機密情報と判定した場合の通知・メニュー表示用ラベル
pub const SENSITIVE_SNIPPET_LABEL: &str = "[機密情報のため非表示]";

/// クイックセレクター表示のデフォルトホットキー
pub const DEFAULT_HOTKEY_QUICK_SELECTOR: &str = "Alt+Shift+S";

/// 通知切替のデフォルトホットキー
pub const DEFAULT_HOTKEY_NOTIFICATION: &str = "Alt+Shift+N";

/// 一時停止切替のデフォルトホットキー
pub const DEFAULT_HOTKEY_PAUSE: &str = "Alt+Shift+P";

/// 終了のデフォルトホットキー
pub const DEFAULT_HOTKEY_QUIT: &str = "Alt+Shift+Q";

/// 加工取り消しのデフォルトホットキー
pub const DEFAULT_HOTKEY_UNDO: &str = "Alt+Shift+Z";

/// 登録クリップセレクター表示のデフォルトホットキー
pub const DEFAULT_HOTKEY_CLIP_SELECTOR: &str = "Alt+Shift+T";

/// 画面 OCR キャプチャのデフォルトホットキー
pub const DEFAULT_HOTKEY_OCR: &str = "Alt+Shift+O";

/// 登録クリップの最大件数
pub const MAX_REGISTERED_CLIPS: usize = 100;

/// 登録クリップラベルの最大文字数
pub const MAX_REGISTERED_CLIP_LABEL_CHARS: usize = 64;

/// 登録クリッププレビューの最大文字数 (UI 表示用)
pub const REGISTERED_CLIP_PREVIEW_MAX_CHARS: usize = 40;

/// 登録画像 PNG の最大バイト数 (16 MiB)
pub const MAX_REGISTERED_IMAGE_BYTES: usize = 16 * 1024 * 1024;

/// 登録画像の最大辺長 (ピクセル)
pub const MAX_REGISTERED_IMAGE_DIMENSION: u32 = 8192;

/// 登録画像の最大ピクセル数 (`4096×4096`)
pub const MAX_REGISTERED_IMAGE_PIXELS: u64 = 16_777_216;

/// セレクター用画像プレビューの最大辺長 (ピクセル)
pub const SELECTOR_IMAGE_PREVIEW_MAX_DIMENSION: u32 = 64;

/// セレクター hover プレビューの最大辺長 (ピクセル)
pub const SELECTOR_IMAGE_HOVER_PREVIEW_MAX_DIMENSION: u32 = 480;

/// セレクター用画像プレビュー JPEG の最大バイト数
pub const MAX_SELECTOR_IMAGE_PREVIEW_BYTES: usize = 24 * 1024;

/// セレクター hover プレビュー JPEG の最大バイト数
pub const MAX_SELECTOR_IMAGE_HOVER_PREVIEW_BYTES: usize = 256 * 1024;

/// お気に入り変換モードの最大件数
pub const MAX_FAVORITE_MODES: usize = 20;

/// 加工パイプラインの最大段数
pub const MAX_PIPELINE_LENGTH: usize = 10;

// ======================================================================
// ヘルパー関数
// ======================================================================
/// Serdeのデフォルト値(true)を返すヘルパー関数
///
/// 設定ファイルのデシリアライズ時に、項目が欠けている場合のデフォルト値(true)を提供する
///
/// # Returns
/// * `bool` - 常に `true`
pub fn default_true() -> bool {
    true
}
