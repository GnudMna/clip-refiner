use crate::config::FavoriteMoveDirection;
use crate::platform::bundled_docs::BundledDocument;
use crate::refiner::RefineMode;

// ======================================================================
// カスタムイベント (ハブ)
// ======================================================================
/// アプリケーション内で発生するカスタムユーザーイベント
///
/// ドメイン別のサブイベントを包む薄いハブ。処理は `user_event` ルーターが振り分ける
#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    /// セレクター UI
    Selector(SelectorEvent),
    /// 登録クリップ
    Clips(ClipsEvent),
    /// お気に入り変換モード
    Favorites(FavoritesEvent),
    /// トレイメニュー再構築
    MenuRefresh(MenuRefreshEvent),
    /// 設定ファイル
    Config(ConfigEvent),
    /// クリップボードワーカー
    Worker(WorkerEvent),
    /// グローバルホットキー
    Hotkey(HotkeyEvent),
    /// 設定ウィンドウ
    Settings(SettingsEvent),
    /// アプリ更新
    Update(UpdateEvent),
    /// 画面 OCR
    Ocr(OcrEvent),
    /// 連続記録
    Record(RecordEvent),
}

// ======================================================================
// セレクター
// ======================================================================
/// クイック / 登録クリップセレクター関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorEvent {
    /// クイックセレクターを表示する
    ShowQuick,
    /// クイックセレクターを非表示にする
    HideQuick,
    /// 登録クリップセレクターを表示する
    ShowClip,
    /// 登録クリップセレクターを非表示にする
    HideClip,
    /// 自動変換モードの変更要求
    RequestModeChange(RefineMode),
    /// 自動変換を変えずに現在のクリップボードへ一回だけ適用する要求
    RequestApplyOnce(RefineMode),
}

// ======================================================================
// 登録クリップ
// ======================================================================
/// 登録クリップ関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipsEvent {
    /// 登録クリップのクリップボードコピー要求
    RequestCopy(usize),
    /// クリップボードの内容を登録クリップとして保存する要求
    RequestRegister,
    /// 登録クリップの削除要求
    RequestDelete(usize),
    /// 登録クリップの表示順変更要求
    RequestMove(usize, FavoriteMoveDirection),
    /// 登録クリップのテンプレート展開の切り替え要求
    RequestSetTemplate(usize, bool),
    /// 登録クリップの `{counter}` の連番を最初に戻す要求
    RequestResetCounter(usize),
    /// 登録クリップセレクターのホバー表示用に本文の全文を返す要求
    RequestFullText(usize),
    /// 設定ウィンドウの編集用に本文の全文を返す要求
    RequestEditOpen(usize),
    /// 設定ウィンドウから預かった編集内容の保存要求
    RequestEditApply,
    /// 登録クリップメニュー / UI の再構築要求
    Refresh,
}

// ======================================================================
// お気に入り
// ======================================================================
/// お気に入り変換モード関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoritesEvent {
    /// お気に入り変換モードの登録切替要求
    RequestToggle(RefineMode),
    /// お気に入り変換モードの表示順変更要求
    RequestMove(RefineMode, FavoriteMoveDirection),
}

// ======================================================================
// トレイメニュー再構築
// ======================================================================
/// トレイメニュー再構築イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRefreshEvent {
    /// 履歴メニューの内容再構築要求
    History,
    /// 連続記録メニューの内容再構築要求
    ContinuousRecord,
}

// ======================================================================
// 設定ファイル
// ======================================================================
/// ディスク上の設定ファイル関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEvent {
    /// 設定ファイルを再読み込みする要求
    Reload,
}

// ======================================================================
// クリップボードワーカー
// ======================================================================
/// クリップボードワーカーの寿命・復旧イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerEvent {
    /// クリップボードワーカーの再起動要求
    Restart,
    /// クリップボードワーカーが停止した
    Stopped,
    /// クリップボードワーカーが稼働を再開した
    Ready,
}

// ======================================================================
// ホットキー
// ======================================================================
/// グローバルホットキー関連イベント
#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    /// システム全体から受信したグローバルホットキーイベント
    Received(global_hotkey::GlobalHotKeyEvent),
    /// グローバルホットキー全体の再登録要求
    ReloadAll,
    /// お気に入り変換モード用ホットキーの再登録要求
    ReloadFavorites,
}

// ======================================================================
// 設定ウィンドウ
// ======================================================================
/// 設定ウィンドウ関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsEvent {
    /// 設定ウィンドウを表示する
    Show,
    /// 設定ウィンドウを非表示にする
    Hide,
    /// 設定ウィンドウからの部分更新パッチを適用する
    ApplyPatch,
    /// 設定ウィンドウからの履歴全削除要求
    ClearHistory,
    /// 設定ウィンドウからの連続記録全削除要求
    ClearContinuousRecord,
    /// 設定ウィンドウからの設定ファイル起動要求
    OpenConfigFile,
    /// 設定ウィンドウからの同梱文書の起動要求
    OpenDocument(BundledDocument),
    /// 設定ウィンドウからの正規表現プレビュー要求
    RegexPreview,
    /// 設定ウィンドウからの更新確認要求
    CheckUpdate,
    /// 設定ウィンドウからの更新適用要求
    ApplyUpdate,
    /// 設定ウィンドウの表示内容を最新化する
    Refresh,
}

// ======================================================================
// アプリ更新
// ======================================================================
/// アプリ更新関連イベント (バックグラウンド処理の完了通知)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateEvent {
    /// バックグラウンドの更新確認が完了した
    CheckFinished,
    /// バックグラウンドの更新適用が完了した
    ApplyFinished,
}

// ======================================================================
// OCR
// ======================================================================
/// 画面 OCR 関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrEvent {
    /// OCR オーバーレイを破棄してアイドル RSS を回収する
    DisposeOverlay,
}

// ======================================================================
// 連続記録
// ======================================================================
/// 連続記録関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordEvent {
    /// 連続記録貼り付け用に `Ctrl+V` を送る (ワーカー書き込み完了後)
    SendPaste,
}

// ======================================================================
// From 実装
// ======================================================================
impl From<SelectorEvent> for AppEvent {
    fn from(event: SelectorEvent) -> Self {
        Self::Selector(event)
    }
}

impl From<ClipsEvent> for AppEvent {
    fn from(event: ClipsEvent) -> Self {
        Self::Clips(event)
    }
}

impl From<FavoritesEvent> for AppEvent {
    fn from(event: FavoritesEvent) -> Self {
        Self::Favorites(event)
    }
}

impl From<MenuRefreshEvent> for AppEvent {
    fn from(event: MenuRefreshEvent) -> Self {
        Self::MenuRefresh(event)
    }
}

impl From<ConfigEvent> for AppEvent {
    fn from(event: ConfigEvent) -> Self {
        Self::Config(event)
    }
}

impl From<WorkerEvent> for AppEvent {
    fn from(event: WorkerEvent) -> Self {
        Self::Worker(event)
    }
}

impl From<HotkeyEvent> for AppEvent {
    fn from(event: HotkeyEvent) -> Self {
        Self::Hotkey(event)
    }
}

impl From<SettingsEvent> for AppEvent {
    fn from(event: SettingsEvent) -> Self {
        Self::Settings(event)
    }
}

impl From<UpdateEvent> for AppEvent {
    fn from(event: UpdateEvent) -> Self {
        Self::Update(event)
    }
}

impl From<OcrEvent> for AppEvent {
    fn from(event: OcrEvent) -> Self {
        Self::Ocr(event)
    }
}

impl From<RecordEvent> for AppEvent {
    fn from(event: RecordEvent) -> Self {
        Self::Record(event)
    }
}
