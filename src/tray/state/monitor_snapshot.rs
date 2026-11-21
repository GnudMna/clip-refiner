use crate::config::RegexSettings;
use crate::refiner::RefineMode;
use crate::security::ContentFingerprint;

/// 監視ループがループ先頭で一括取得する設定スナップショット
///
/// 1ループあたり `config` `RwLock` の取得を1回に削減するために使用する
pub struct MonitorSnapshot {
    /// 監視時に適用する加工モード列
    pub pipeline: Vec<RefineMode>,
    /// ポーリング間隔(ミリ秒)
    pub interval_ms: u64,
    /// 一時停止中かどうか
    pub is_paused: bool,
    /// クリップボード履歴が有効かどうか
    pub history_enabled: bool,
    /// 正規表現加工用の設定
    pub regex_settings: RegexSettings,
}

impl MonitorSnapshot {
    /// パイプライン末尾が画像出力モードかどうか
    pub fn produces_image(&self) -> bool {
        self.pipeline
            .last()
            .is_some_and(|mode| mode.produces_image())
    }
}

/// 監視ループにおける二重加工防止用の状態
///
/// クリップボード本文は平文で保持せず、指紋のみを記録する
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProcessedState {
    /// ポーリングで前回観測したクリップボード本文の指紋
    pub last_seen: ContentFingerprint,
    /// 直近の加工で書き戻した本文の指紋 (自身の変更イベントを1回無視)
    pub last_written: Option<ContentFingerprint>,
}

impl ProcessedState {
    /// 指定テキストが `last_seen` と一致するか判定する
    pub fn matches_last_seen(&self, text: &str) -> bool {
        self.last_seen.matches(text)
    }

    /// 指定テキストが `last_written` と一致するか判定する
    pub fn matches_last_written(&self, text: &str) -> bool {
        self.last_written.is_some_and(|fp| fp.matches(text))
    }
}
