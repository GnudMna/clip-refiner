// ======================================================================
// アプリーケーション情報
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

/// クイックセレクター表示のデフォルトホットキー
pub const DEFAULT_HOTKEY_SELECTOR: &str = "Alt+Shift+S";

/// 通知切替のデフォルトホットキー
pub const DEFAULT_HOTKEY_NOTIFICATION: &str = "Alt+Shift+N";

/// 一時停止切替のデフォルトホットキー
pub const DEFAULT_HOTKEY_PAUSE: &str = "Alt+Shift+P";

/// 終了のデフォルトホットキー
pub const DEFAULT_HOTKEY_QUIT: &str = "Alt+Shift+Q";

/// 加工取り消しのデフォルトホットキー
pub const DEFAULT_HOTKEY_UNDO: &str = "Alt+Shift+Z";

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
