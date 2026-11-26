use std::fmt;

use serde::{Deserialize, Deserializer};

use super::super::model::SettingsPatch;
use crate::config::{FavoriteMoveDirection, RegexSettings};
use crate::platform::bundled_docs::BundledDocument;
use crate::refiner::RefineMode;
use crate::security::{SecretString, secret_from};
use crate::tray::state::ClipRef;

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
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
    },
    /// 登録クリップを削除する
    ClipDelete {
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
    },
    /// 登録クリップの表示順を変更する
    ClipMove {
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
        /// 移動方向 (`up` / `down`)
        direction: IpcFavoriteMoveDirection,
    },
    /// 登録クリップのテンプレート展開を切り替える
    ClipTemplate {
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
        /// テンプレートとして展開するかどうか
        enabled: bool,
    },
    /// 登録クリップの `{counter}` の連番を最初に戻す
    ClipCounterReset {
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
    },
    /// 登録クリップの編集用に本文を要求する
    ClipEditOpen {
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
    },
    /// 登録クリップのラベルと本文を更新する
    ClipEdit {
        /// 操作対象の登録クリップ
        #[serde(flatten)]
        clip: IpcClipTarget,
        /// 新しい表示ラベル
        label: String,
        /// 新しい本文
        text: IpcSecretText,
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
    /// 更新プログラムを起動し、アプリ更新を確認する
    CheckUpdate,
}

/// IPC で受け取る登録クリップの指定 (表示時のインデックスと識別トークン)
///
/// トークンは必須で、形式が不正なメッセージは解釈しない
#[derive(Debug, Clone, Copy, Deserialize)]
struct IpcClipTarget {
    /// 登録クリップのインデックス
    index: usize,
    /// 表示時点の識別トークン (16 桁の 16 進数)
    #[serde(deserialize_with = "deserialize_clip_token")]
    token: u64,
}

impl From<IpcClipTarget> for ClipRef {
    fn from(target: IpcClipTarget) -> Self {
        Self::checked(target.index, target.token)
    }
}

/// 識別トークンの文字列を数値へ戻す
fn deserialize_clip_token<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;
    ClipRef::parse_token(&text)
        .ok_or_else(|| serde::de::Error::custom("識別トークンの形式が不正です"))
}

/// IPC で受け取る本文 (破棄時にゼロ化し、`Debug` では内容を出さない)
#[derive(Clone)]
pub(crate) struct IpcSecretText(pub(crate) SecretString);

impl fmt::Debug for IpcSecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IpcSecretText(...)")
    }
}

impl<'de> Deserialize<'de> for IpcSecretText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|text| Self(secret_from(text)))
    }
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
    ClipCopy(ClipRef),
    /// 登録クリップを削除する
    ClipDelete(ClipRef),
    /// 登録クリップの表示順を変更する
    ClipMove(ClipRef, FavoriteMoveDirection),
    /// 登録クリップのテンプレート展開を切り替える
    ClipTemplate(ClipRef, bool),
    /// 登録クリップの `{counter}` の連番を最初に戻す
    ClipCounterReset(ClipRef),
    /// 登録クリップの編集用に本文を要求する
    ClipEditOpen(ClipRef),
    /// 登録クリップのラベルと本文を更新する
    ClipEdit(ClipRef, String, IpcSecretText),
    /// クリップボード内容を登録クリップとして追加する
    ClipRegister,
    /// お気に入り変換モードの登録状態を切り替える
    FavoriteToggle(RefineMode),
    /// お気に入り変換モードの表示順を変更する
    FavoriteMove(RefineMode, FavoriteMoveDirection),
    /// 正規表現置換のプレビューを要求する
    RegexPreview(u64, RegexSettings, String),
    /// 更新プログラムを起動し、アプリ更新を確認する
    CheckUpdate,
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
            SettingsIpcMessage::ClipCopy { clip } => Self::ClipCopy(clip.into()),
            SettingsIpcMessage::ClipDelete { clip } => Self::ClipDelete(clip.into()),
            SettingsIpcMessage::ClipMove { clip, direction } => {
                Self::ClipMove(clip.into(), direction.into())
            }
            SettingsIpcMessage::ClipTemplate { clip, enabled } => {
                Self::ClipTemplate(clip.into(), enabled)
            }
            SettingsIpcMessage::ClipCounterReset { clip } => Self::ClipCounterReset(clip.into()),
            SettingsIpcMessage::ClipEditOpen { clip } => Self::ClipEditOpen(clip.into()),
            SettingsIpcMessage::ClipEdit { clip, label, text } => {
                Self::ClipEdit(clip.into(), label, text)
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
        }
    }
}

/// IPC メッセージ文字列 (JSON) を解釈する
pub(crate) fn parse_settings_ipc_message(msg: &str) -> Option<SettingsIpcAction> {
    serde_json::from_str::<SettingsIpcMessage>(msg)
        .ok()
        .map(SettingsIpcAction::from)
}
