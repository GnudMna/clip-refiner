use serde::Deserialize;

use super::super::model::SettingsPatch;
use crate::config::{FavoriteMoveDirection, RegexSettings};
use crate::platform::bundled_docs::BundledDocument;
use crate::refiner::RefineMode;

// ======================================================================
// IPC メッセージ
// ======================================================================
/// 設定ウィンドウから送られる JSON IPC メッセージ
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum SettingsIpcMessage {
    /// ウィンドウを閉じる
    Close,
    /// 設定ファイルを既定アプリで開く
    OpenConfig,
    /// 同梱文書を既定アプリで開く
    OpenDocument {
        /// 開く文書
        target: IpcDocumentTarget,
    },
    /// 設定の一部を適用する
    Apply {
        /// 部分更新パッチ
        patch: Box<SettingsPatch>,
    },
    /// 履歴を全削除する
    ClearHistory,
    /// 連続記録を全削除する
    ClearRecord,
    /// 登録クリップをクリップボードへコピーする
    ClipCopy {
        /// 登録クリップのインデックス
        index: usize,
    },
    /// 登録クリップを削除する
    ClipDelete {
        /// 登録クリップのインデックス
        index: usize,
    },
    /// 登録クリップの表示順を変更する
    ClipMove {
        /// 登録クリップのインデックス
        index: usize,
        /// 移動方向 (`up` / `down`)
        direction: IpcFavoriteMoveDirection,
    },
    /// 登録クリップのテンプレート展開を切り替える
    ClipTemplate {
        /// 登録クリップのインデックス
        index: usize,
        /// テンプレートとして展開するかどうか
        enabled: bool,
    },
    /// クリップボード内容を登録クリップとして追加する
    ClipRegister,
    /// お気に入り変換モードの登録状態を切り替える
    FavoriteToggle {
        /// 対象の変換モード
        mode: RefineMode,
    },
    /// お気に入り変換モードの表示順を変更する
    FavoriteMove {
        /// 対象の変換モード
        mode: RefineMode,
        /// 移動方向 (`up` / `down`)
        direction: IpcFavoriteMoveDirection,
    },
    /// 正規表現置換のプレビューを要求する
    RegexPreview {
        /// クライアント側リクエスト ID
        #[serde(rename = "requestId")]
        request_id: u64,
        /// プレビュー対象の正規表現設定
        settings: RegexSettings,
        /// プレビューに使うサンプル文字列
        #[serde(default)]
        sample: String,
    },
    /// アプリ更新を確認する
    CheckUpdate,
    /// 確認済みの更新をダウンロードして適用する
    ApplyUpdate,
}

/// IPC 向けの同梱文書の指定
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum IpcDocumentTarget {
    /// 利用者向けドキュメント
    UserDocs,
    /// 本体のライセンス (`LICENSE`)
    License,
    /// 第三者ライセンス表示 (`THIRD_PARTY_NOTICES`)
    ThirdPartyNotices,
}

impl From<IpcDocumentTarget> for BundledDocument {
    fn from(target: IpcDocumentTarget) -> Self {
        match target {
            IpcDocumentTarget::UserDocs => Self::UserDocs,
            IpcDocumentTarget::License => Self::License,
            IpcDocumentTarget::ThirdPartyNotices => Self::ThirdPartyNotices,
        }
    }
}

/// IPC 向けの表示順の移動方向 (お気に入り・登録クリップ共通)
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IpcFavoriteMoveDirection {
    /// 上へ移動
    Up,
    /// 下へ移動
    Down,
}

impl From<IpcFavoriteMoveDirection> for FavoriteMoveDirection {
    fn from(direction: IpcFavoriteMoveDirection) -> Self {
        match direction {
            IpcFavoriteMoveDirection::Up => Self::Up,
            IpcFavoriteMoveDirection::Down => Self::Down,
        }
    }
}

// ======================================================================
// IPC アクション
// ======================================================================
/// 設定ウィンドウから送られる IPC メッセージの種別
#[derive(Debug, Clone)]
pub(crate) enum SettingsIpcAction {
    /// ウィンドウを閉じる
    Close,
    /// 設定ファイルを既定アプリで開く
    OpenConfigFile,
    /// 同梱文書を既定アプリで開く
    OpenDocument(BundledDocument),
    /// 設定の一部を適用する
    Apply(Box<SettingsPatch>),
    /// 履歴を全削除する
    ClearHistory,
    /// 連続記録を全削除する
    ClearContinuousRecord,
    /// 登録クリップをクリップボードへコピーする
    ClipCopy(usize),
    /// 登録クリップを削除する
    ClipDelete(usize),
    /// 登録クリップの表示順を変更する
    ClipMove(usize, FavoriteMoveDirection),
    /// 登録クリップのテンプレート展開を切り替える
    ClipTemplate(usize, bool),
    /// クリップボード内容を登録クリップとして追加する
    ClipRegister,
    /// お気に入り変換モードの登録状態を切り替える
    FavoriteToggle(RefineMode),
    /// お気に入り変換モードの表示順を変更する
    FavoriteMove(RefineMode, FavoriteMoveDirection),
    /// 正規表現置換のプレビューを要求する
    RegexPreview(u64, RegexSettings, String),
    /// アプリ更新を確認する
    CheckUpdate,
    /// 確認済みの更新をダウンロードして適用する
    ApplyUpdate,
}

impl From<SettingsIpcMessage> for SettingsIpcAction {
    fn from(message: SettingsIpcMessage) -> Self {
        match message {
            SettingsIpcMessage::Close => Self::Close,
            SettingsIpcMessage::OpenConfig => Self::OpenConfigFile,
            SettingsIpcMessage::OpenDocument { target } => Self::OpenDocument(target.into()),
            SettingsIpcMessage::Apply { patch } => Self::Apply(patch),
            SettingsIpcMessage::ClearHistory => Self::ClearHistory,
            SettingsIpcMessage::ClearRecord => Self::ClearContinuousRecord,
            SettingsIpcMessage::ClipCopy { index } => Self::ClipCopy(index),
            SettingsIpcMessage::ClipDelete { index } => Self::ClipDelete(index),
            SettingsIpcMessage::ClipMove { index, direction } => {
                Self::ClipMove(index, direction.into())
            }
            SettingsIpcMessage::ClipTemplate { index, enabled } => {
                Self::ClipTemplate(index, enabled)
            }
            SettingsIpcMessage::ClipRegister => Self::ClipRegister,
            SettingsIpcMessage::FavoriteToggle { mode } => Self::FavoriteToggle(mode),
            SettingsIpcMessage::FavoriteMove { mode, direction } => {
                Self::FavoriteMove(mode, direction.into())
            }
            SettingsIpcMessage::RegexPreview {
                request_id,
                settings,
                sample,
            } => Self::RegexPreview(request_id, settings, sample),
            SettingsIpcMessage::CheckUpdate => Self::CheckUpdate,
            SettingsIpcMessage::ApplyUpdate => Self::ApplyUpdate,
        }
    }
}

/// IPC メッセージ文字列 (JSON) を解釈する
pub(crate) fn parse_settings_ipc_message(msg: &str) -> Option<SettingsIpcAction> {
    serde_json::from_str::<SettingsIpcMessage>(msg)
        .ok()
        .map(SettingsIpcAction::from)
}
