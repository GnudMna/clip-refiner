use tao::window::WindowId;

use crate::config::{AppConfig, FavoriteMoveDirection};
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
    /// `WebView2` のプロセス
    WebView(WebViewEvent),
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
/// 操作対象の登録クリップの指定
///
/// 一覧内の位置 (`index`) に加え、設定ウィンドウからの操作では表示時点の識別トークンを持つ
/// トークンがある場合は実行時の一覧と照合し、表示後に並びや内容が変わっていれば別のクリップを操作しない
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipRef {
    /// 登録クリップのインデックス
    pub index: usize,
    /// 表示時点の識別トークン (`RegisteredClip::identity_token`)。`None` は照合しない
    pub token: Option<u64>,
}

impl ClipRef {
    /// 識別トークンで照合しない指定を生成する
    ///
    /// # Arguments
    /// * `index` - 登録クリップのインデックス
    ///
    /// # Returns
    /// * `Self` - インデックスのみの指定
    pub fn unchecked(index: usize) -> Self {
        Self { index, token: None }
    }

    /// 識別トークンで照合する指定を生成する
    ///
    /// # Arguments
    /// * `index` - 登録クリップのインデックス
    /// * `token` - 表示時点の識別トークン
    ///
    /// # Returns
    /// * `Self` - 照合付きの指定
    pub fn checked(index: usize, token: u64) -> Self {
        Self {
            index,
            token: Some(token),
        }
    }

    /// 識別トークンを UI とやり取りする文字列 (16 桁の 16 進数) にする
    ///
    /// JavaScript の数値では `u64` を正確に扱えないため文字列で渡す
    ///
    /// # Arguments
    /// * `token` - 識別トークン
    ///
    /// # Returns
    /// * `String` - 16 桁の 16 進数
    pub fn format_token(token: u64) -> String {
        format!("{token:016x}")
    }

    /// `format_token` で作った文字列を識別トークンへ戻す
    ///
    /// # Arguments
    /// * `text` - 16 桁の 16 進数
    ///
    /// # Returns
    /// * `Option<u64>` - 形式が正しい場合は `Some(token)`
    pub fn parse_token(text: &str) -> Option<u64> {
        if text.len() != 16 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u64::from_str_radix(text, 16).ok()
    }

    /// 現在の一覧で指しているインデックスを返す
    ///
    /// # Arguments
    /// * `self` - 対象の指定
    /// * `config` - 照合先の設定
    ///
    /// # Returns
    /// * `Option<usize>` - 範囲内でトークンが一致する (または照合しない) 場合は `Some(index)`
    pub fn resolve(&self, config: &AppConfig) -> Option<usize> {
        let clip = config.clips.get(self.index)?;
        match self.token {
            Some(token) if clip.identity_token() != token => None,
            _ => Some(self.index),
        }
    }
}

/// 登録クリップ関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipsEvent {
    /// 登録クリップのクリップボードコピー要求
    RequestCopy(ClipRef),
    /// クリップボードの内容を登録クリップとして保存する要求
    RequestRegister,
    /// 登録クリップの削除要求
    RequestDelete(ClipRef),
    /// 登録クリップの表示順変更要求
    RequestMove(ClipRef, FavoriteMoveDirection),
    /// 登録クリップのテンプレート展開の切り替え要求
    RequestSetTemplate(ClipRef, bool),
    /// 登録クリップの `{counter}` の連番を最初に戻す要求
    RequestResetCounter(ClipRef),
    /// 登録クリップセレクターのホバー表示用に本文の全文を返す要求
    RequestFullText(usize),
    /// 設定ウィンドウの編集用に本文の全文を返す要求
    RequestEditOpen(ClipRef),
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
    /// 設定ウィンドウの表示内容を最新化する
    Refresh,
}

// ======================================================================
// アプリ更新
// ======================================================================
/// アプリ更新関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateEvent {
    /// 更新プログラムからアプリの終了を依頼された
    ExitRequested,
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
// WebView
// ======================================================================
/// `WebView2` のプロセス関連イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebViewEvent {
    /// ブラウザ / 描画プロセスが異常終了した。該当ウィンドウの `WebView` を破棄し、次の表示で作り直す
    ///
    /// 破棄・作り直し後の古いウィンドウ ID は一致しないため無視される
    ProcessFailed(WindowId),
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

impl From<WebViewEvent> for AppEvent {
    fn from(event: WebViewEvent) -> Self {
        Self::WebView(event)
    }
}
