use crate::config::FavoriteMoveDirection;
use crate::refiner::RefineMode;

// ======================================================================
// カスタムイベント
// ======================================================================
/// アプリケーション内で発生するカスタムユーザーイベント
#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    /// クリップボード加工モードの変更要求
    RequestModeChange(RefineMode),
    /// クイックセレクター (加工モード選択) の非表示要求
    HideQuickSelector,
    /// 登録クリップセレクターの非表示要求
    HideClipSelector,
    /// 登録クリップのクリップボードコピー要求
    RequestClipCopy(usize),
    /// クリップボードの内容を登録クリップとして保存する要求
    RequestClipRegister,
    /// 登録クリップの削除要求
    RequestClipDelete(usize),
    /// お気に入り変換モードの登録切替要求
    RequestFavoriteToggle(RefineMode),
    /// お気に入り変換モードの表示順変更要求
    RequestFavoriteMove(RefineMode, FavoriteMoveDirection),
    /// 履歴メニューの内容再構築要求
    RefreshHistory,
    /// 登録クリップメニューの内容再構築要求
    RefreshClips,
    /// ディスク上の設定ファイルを再読み込みする要求
    ReloadConfig,
    /// お気に入り変換モード用ホットキーの再登録要求
    ReloadFavoriteHotkeys,
    /// システム全体から受信したグローバルホットキーイベント
    Hotkey(global_hotkey::GlobalHotKeyEvent),
}
