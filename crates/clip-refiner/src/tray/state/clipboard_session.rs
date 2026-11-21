use std::sync::Mutex;

use super::lock_ext::LockExt;
use super::monitor_snapshot::ProcessedState;

use crate::security::{SecretString, secret_from};
use crate::tray::history_store::{EncryptedHistoryStore, EncryptedRecordStore};

// ======================================================================
// クリップボードセッション状態
// ======================================================================
/// セッション限定の監視・取り消し・履歴・連続記録をまとめた状態
///
/// プロセス再起動で破棄される揮発データのみを保持する
pub(crate) struct ClipboardSessionState {
    processed_state: Mutex<ProcessedState>,
    undo_text: Mutex<Option<SecretString>>,
    history_store: Mutex<EncryptedHistoryStore>,
    continuous_record_store: Mutex<EncryptedRecordStore>,
}

impl ClipboardSessionState {
    /// 空のセッション状態を生成する
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 暗号化ストアの鍵生成に失敗した場合は `Err`
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            processed_state: Mutex::new(ProcessedState::default()),
            undo_text: Mutex::new(None),
            history_store: Mutex::new(EncryptedHistoryStore::new()?),
            continuous_record_store: Mutex::new(EncryptedRecordStore::new()?),
        })
    }

    /// 二重加工防止状態を更新する
    ///
    /// # Arguments
    /// * `f` - `ProcessedState` へ排他アクセスして変更を行うクロージャ
    ///
    /// # Returns
    /// * `R` - クロージャ `f` の戻り値
    pub fn with_processed_state<R>(&self, f: impl FnOnce(&mut ProcessedState) -> R) -> R {
        f(&mut self.processed_state.lock_ignore_poison())
    }

    /// 加工成功後にクリップボードへ書き戻したことを記録する
    ///
    /// # Arguments
    /// * `output` - 書き戻した加工後テキスト
    pub fn record_processing_success(&self, output: &str) {
        self.with_processed_state(|ps| {
            let fp = crate::security::ContentFingerprint::from_text(output);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 画像加工成功後に元テキストの指紋を記録する
    ///
    /// クリップボード上に TSV が残る場合の再加工ループを防ぐ
    ///
    /// # Arguments
    /// * `source_text` - 加工前の元テキスト
    pub fn record_image_processing_success(&self, source_text: &str) {
        self.with_processed_state(|ps| {
            let fp = crate::security::ContentFingerprint::from_text(source_text);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 加工せずに観測したクリップボード本文を記録する
    ///
    /// # Arguments
    /// * `text` - 観測したクリップボード本文
    pub fn record_clipboard_observed(&self, text: &str) {
        self.with_processed_state(|ps| {
            ps.last_seen = crate::security::ContentFingerprint::from_text(text);
        });
    }

    /// 履歴復元など、外部からクリップボードへ設定した本文を記録する
    ///
    /// # Arguments
    /// * `text` - クリップボードへ設定した本文
    pub fn record_clipboard_set(&self, text: &str) {
        self.with_processed_state(|ps| {
            let fp = crate::security::ContentFingerprint::from_text(text);
            ps.last_written = Some(fp);
            ps.last_seen = fp;
        });
    }

    /// 加工成功時に取り消し用の元テキストを記録する
    ///
    /// # Arguments
    /// * `text` - 取り消し用に保持する加工前テキスト
    pub fn record_undo_source(&self, text: &str) {
        *self.undo_text.lock_ignore_poison() = Some(secret_from(text));
    }

    /// 取り消し用の元テキストを取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 取り消し可能な加工があれば `Some(元テキスト)`
    pub fn take_undo_source(&self) -> Option<SecretString> {
        self.undo_text.lock_ignore_poison().take()
    }

    /// 履歴の件数を返す
    ///
    /// # Returns
    /// * `usize` - 保持している履歴件数
    pub fn history_len(&self) -> usize {
        self.history_store.lock_ignore_poison().len()
    }

    /// 指定インデックスの履歴を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 履歴ストア内のインデックス (0 が最新)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_history_entry(&self, index: usize) -> Option<SecretString> {
        self.history_store.lock_ignore_poison().entry_at(index)
    }

    /// 履歴をクリアする
    pub fn clear_history(&self) {
        self.history_store.lock_ignore_poison().clear();
    }

    /// クリップボード履歴に新しいテキストを追加する
    ///
    /// 空文字やトリム後に空になる文字列は無視される
    ///
    /// # Arguments
    /// * `text` - 履歴へ追加するクリップボード本文
    /// * `limit` - 保持する最大件数
    ///
    /// # Returns
    /// * `bool` - 追加された場合は `true`
    pub fn add_to_history(&self, text: impl AsRef<str>, limit: usize) -> bool {
        let text = text.as_ref();
        if text.trim().is_empty() {
            return false;
        }
        self.history_store.lock_ignore_poison().add(text, limit);
        true
    }

    /// 連続記録の件数を返す
    ///
    /// # Returns
    /// * `usize` - 保持している連続記録件数
    pub fn continuous_record_len(&self) -> usize {
        self.continuous_record_store.lock_ignore_poison().len()
    }

    /// 指定インデックスの連続記録を復号して取得する
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 復号成功時は `Some(本文)`、範囲外や復号失敗時は `None`
    pub fn get_continuous_record_entry(&self, index: usize) -> Option<SecretString> {
        self.continuous_record_store
            .lock_ignore_poison()
            .entry_at(index)
    }

    /// 連続記録をクリアする
    pub fn clear_continuous_record(&self) {
        self.continuous_record_store.lock_ignore_poison().clear();
    }

    /// 連続記録に新しいテキストをキューへ積む
    ///
    /// 空文字やトリム後に空になる文字列は無視される
    ///
    /// # Arguments
    /// * `text` - 連続記録へ追加するクリップボード本文
    /// * `limit` - 保持する最大件数
    ///
    /// # Returns
    /// * `bool` - 追加された場合は `true`
    pub fn enqueue_continuous_record(&self, text: impl AsRef<str>, limit: usize) -> bool {
        let text = text.as_ref();
        if text.trim().is_empty() {
            return false;
        }
        self.continuous_record_store
            .lock_ignore_poison()
            .enqueue(text, limit);
        true
    }

    /// 連続記録キュー先頭を取り出す
    ///
    /// # Returns
    /// * `Option<SecretString>` - 先頭エントリ。空の場合は `None`
    pub fn discharge_continuous_record_front(&self) -> Option<SecretString> {
        self.continuous_record_store
            .lock_ignore_poison()
            .discharge_front()
    }

    /// 連続記録キューから指定位置のエントリを取り出す
    ///
    /// # Arguments
    /// * `index` - 連続記録キュー内のインデックス (0 が先頭)
    ///
    /// # Returns
    /// * `Option<SecretString>` - 排出した本文。範囲外の場合は `None`
    pub fn discharge_continuous_record_at(&self, index: usize) -> Option<SecretString> {
        self.continuous_record_store
            .lock_ignore_poison()
            .discharge_at(index)
    }
}
