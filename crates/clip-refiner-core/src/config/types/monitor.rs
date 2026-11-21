use serde::{Deserialize, Serialize};

// ======================================================================
// 監視モード
// ======================================================================
/// クリップボードの監視方式
///
/// クリップボードの更新を検知するための異なるアプローチを提供する
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    /// 一定間隔でクリップボードの内容を確認するポーリング方式
    Polling,
    /// OS のクリップボード変更通知 (`AddClipboardFormatListener`) で起床する方式
    /// 変更時のみ本文を読み取り、アイドル時の周期 peek を避ける
    #[default]
    Event,
}
