use crate::config::MonitorMode;
use crate::refiner::RefineMode;
use crate::security::ContentFingerprint;

/// 監視ループがループ先頭で一括取得する設定スナップショット
///
/// 1ループあたり `config` `RwLock` の取得を1回に削減するために使用する。
/// `bool` 項目は `AppConfig` の同名キーをそのまま写す
#[allow(
    clippy::struct_excessive_bools,
    reason = "`bool` 項目は `AppConfig` の同名キーをそのまま写す"
)]
pub struct MonitorSnapshot {
    /// 監視時に適用する変換モード列
    pub pipeline: Vec<RefineMode>,
    /// ポーリング間隔(ミリ秒)
    pub interval_ms: u64,
    /// 一時停止中かどうか
    pub is_paused: bool,
    /// コピー時の自動変換が有効かどうか
    pub auto_refine_enabled: bool,
    /// クリップボード履歴が有効かどうか
    pub history_enabled: bool,
    /// 連続記録モードが有効かどうか
    pub continuous_record_enabled: bool,
    /// 設定上の監視方式 (フォールバック前)
    pub monitor_mode: MonitorMode,
}

impl MonitorSnapshot {
    /// パイプライン末尾が画像出力モードかどうか
    ///
    /// # Arguments
    /// * `self` - この `MonitorSnapshot`
    ///
    /// # Returns
    /// * `bool` - 末尾モードが画像出力の場合は `true`
    pub fn produces_image(&self) -> bool {
        self.pipeline
            .last()
            .is_some_and(|mode| mode.produces_image())
    }

    /// コピー監視による自動加工が有効かどうか
    ///
    /// 自動変換が無効なとき・連続記録モード中・一時停止中は `false`
    ///
    /// # Arguments
    /// * `self` - この `MonitorSnapshot`
    ///
    /// # Returns
    /// * `bool` - 自動加工が有効な場合は `true`
    pub fn auto_processing_enabled(&self) -> bool {
        self.auto_refine_enabled && !self.is_paused && !self.continuous_record_enabled
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
    ///
    /// # Arguments
    /// * `text` - 比較対象のクリップボード本文
    ///
    /// # Returns
    /// * `bool` - `last_seen` と一致する場合は `true`
    pub fn matches_last_seen(&self, text: &str) -> bool {
        self.last_seen.matches(text)
    }

    /// 指定テキストが `last_written` と一致するか判定する
    ///
    /// # Arguments
    /// * `text` - 比較対象のクリップボード本文
    ///
    /// # Returns
    /// * `bool` - `last_written` と一致する場合は `true`
    pub fn matches_last_written(&self, text: &str) -> bool {
        self.last_written.is_some_and(|fp| fp.matches(text))
    }
}
