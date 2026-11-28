use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use crate::refiner::{RefineContext, RefineMode};
use crate::security::SecretString;

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
    /// CLI ワンショット要求を監視エコー回避つきで加工する
    ProcessOneshotPipeline {
        /// 適用する変換モード列
        pipeline: Vec<RefineMode>,
        /// CLI 側で解決済みの加工コンテキスト
        refine_ctx: RefineContext,
        /// 結果を返すチャネル
        reply: Sender<crate::ipc::OneshotProcessResult>,
        /// 依頼側が応答待ちを打ち切ったときに立てるフラグ。立っていれば加工せず破棄する
        cancelled: Arc<AtomicBool>,
    },
    /// 直近の加工を取り消し、加工前のテキストをクリップボードへ復元する
    Undo,
    /// クリップボードの内容を登録クリップとして保存する
    RegisterClipFromClipboard,
    /// OCR 結果をクリップボードへ書き込む
    SetOcrText(SecretString),
    /// 連続記録キューから指定位置のエントリを取り出してクリップボードへセットする
    DischargeContinuousRecordAt(usize),
    /// 連続記録キュー先頭をクリップボードへ載せ、貼り付けキー送信をメインへ依頼する
    PasteContinuousRecordFront,
    /// OS のクリップボード変更通知を受けた (監視ティックの起床用)
    ClipboardChanged,
    /// 設定ディレクトリの変更通知を受けた (外部編集の再読み込み用)
    ConfigFileChanged,
    /// 監視世代や監視周期が変わった (コマンド待ちの時間を計算し直す起床用)
    Wake,
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
            Self::ProcessOneshotPipeline { pipeline, .. } => f
                .debug_struct("ProcessOneshotPipeline")
                .field("pipeline", pipeline)
                .finish_non_exhaustive(),
            Self::Undo => f.write_str("Undo"),
            Self::RegisterClipFromClipboard => f.write_str("RegisterClipFromClipboard"),
            Self::SetOcrText(_) => f.debug_tuple("SetOcrText").field(&"...").finish(),
            Self::DischargeContinuousRecordAt(index) => f
                .debug_tuple("DischargeContinuousRecordAt")
                .field(index)
                .finish(),
            Self::PasteContinuousRecordFront => f.write_str("PasteContinuousRecordFront"),
            Self::ClipboardChanged => f.write_str("ClipboardChanged"),
            Self::ConfigFileChanged => f.write_str("ConfigFileChanged"),
            Self::Wake => f.write_str("Wake"),
            Self::Shutdown => f.write_str("Shutdown"),
        }
    }
}
