use crate::security::SecretString;

use crate::refiner::RefineMode;

// ======================================================================
// コマンド定義
// ======================================================================
/// UI・監視ループからクリップボードワーカーへ送られる操作コマンド
#[derive(Clone)]
pub enum ClipboardCommand {
    /// 指定されたテキストをクリップボードにセットする(履歴からの復元用など)
    SetText(SecretString),
    /// 登録済みクリップボード内容をコピーする
    CopyRegisteredClip(usize),
    /// 現在のクリップボード内容を指定されたモードで加工する
    ProcessMode(RefineMode),
    /// 直近の加工を取り消し、加工前のテキストをクリップボードへ復元する
    Undo,
    /// クリップボードの内容を登録クリップとして保存する
    RegisterClipFromClipboard,
    /// OCR 結果をクリップボードへ書き込む
    SetOcrText(SecretString),
    /// ワーカースレッドの終了要求
    Shutdown,
}

impl std::fmt::Debug for ClipboardCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetText(_) => f.debug_tuple("SetText").field(&"...").finish(),
            Self::CopyRegisteredClip(index) => {
                f.debug_tuple("CopyRegisteredClip").field(&index).finish()
            }
            Self::ProcessMode(mode) => f.debug_tuple("ProcessMode").field(mode).finish(),
            Self::Undo => f.write_str("Undo"),
            Self::RegisterClipFromClipboard => f.write_str("RegisterClipFromClipboard"),
            Self::SetOcrText(_) => f.debug_tuple("SetOcrText").field(&"...").finish(),
            Self::Shutdown => f.write_str("Shutdown"),
        }
    }
}
