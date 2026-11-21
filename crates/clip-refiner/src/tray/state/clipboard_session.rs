use std::sync::Mutex;

use super::lock_ext::LockExt;
use super::monitor_snapshot::ProcessedState;
use crate::security::{ContentFingerprint, SecretString, secret_from};
use crate::tray::history_store::{EncryptedHistoryStore, EncryptedRecordStore};

// ======================================================================
// 取り消し用エントリ
// ======================================================================
/// 取り消し用に保持する加工前テキストと、加工直後のクリップボード本文の指紋
pub(crate) struct UndoEntry {
    /// 取り消し時に書き戻す加工前テキスト
    pub source: SecretString,
    /// 加工直後にクリップボードへ載っているはずの本文の指紋
    written: ContentFingerprint,
    /// 本文を読めない状態を変更済みとみなす場合は `true`
    text_required: bool,
}

/// 取り消し用エントリの取り出し結果
pub(crate) enum UndoLookup {
    /// 取り消せる
    Ready(UndoEntry),
    /// 加工後にクリップボードが変更されたため破棄した
    Stale,
    /// 取り消し用の記録がない
    Empty,
}

// ======================================================================
// クリップボードセッション状態
// ======================================================================
/// セッション限定の監視・取り消し・履歴・連続記録をまとめた状態
///
/// プロセス再起動で破棄される揮発データのみを保持する
pub(crate) struct ClipboardSessionState {
    processed_state: Mutex<ProcessedState>,
    undo_text: Mutex<Option<UndoEntry>>,
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
    /// * `source` - 取り消し用に保持する加工前テキスト
    /// * `written` - 加工によりクリップボードへ書き戻したテキスト
    pub fn record_undo_source(&self, source: &str, written: &str) {
        self.store_undo_entry(UndoEntry {
            source: secret_from(source),
            written: ContentFingerprint::from_text(written),
            text_required: true,
        });
    }

    /// 画像加工成功時に取り消し用の元テキストを記録する
    ///
    /// 画像書き込みでテキストが消える場合があるため、クリップボードからテキストを
    /// 読めない状態は加工直後のままとみなす
    ///
    /// # Arguments
    /// * `source` - 取り消し用に保持する加工前テキスト
    pub fn record_image_undo_source(&self, source: &str) {
        self.store_undo_entry(UndoEntry {
            source: secret_from(source),
            written: ContentFingerprint::from_text(source),
            text_required: false,
        });
    }

    /// 取り消し用エントリを保持する
    ///
    /// # Arguments
    /// * `entry` - 保持する取り消し用エントリ
    pub fn store_undo_entry(&self, entry: UndoEntry) {
        *self.undo_text.lock_ignore_poison() = Some(entry);
    }

    /// 取り消し用の元テキストを取り出す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<SecretString>` - 値がある場合は `Some`、ない場合は `None`
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn take_undo_source(&self) -> Option<SecretString> {
        self.undo_text
            .lock_ignore_poison()
            .take()
            .map(|entry| entry.source)
    }

    /// 現在のクリップボード本文が加工直後のままなら取り消し用エントリを取り出す
    ///
    /// 加工後に別の内容がコピーされていた場合は、上書きを避けるためエントリを破棄する
    ///
    /// # Arguments
    /// * `current` - 現在のクリップボード本文。テキストを読めない場合は `None`
    ///
    /// # Returns
    /// * `UndoLookup` - 取り消せる場合は `Ready`、記録がなければ `Empty`、
    ///   クリップボードが変更済みなら `Stale`
    pub fn take_undo_entry_if_current(&self, current: Option<&str>) -> UndoLookup {
        let Some(entry) = self.undo_text.lock_ignore_poison().take() else {
            return UndoLookup::Empty;
        };
        let is_current = match current {
            Some(text) => entry.written.matches(text),
            None => !entry.text_required,
        };
        if is_current {
            UndoLookup::Ready(entry)
        } else {
            UndoLookup::Stale
        }
    }

    /// 履歴の件数を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `usize` - 数値
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

    /// 履歴をすべて復号して先頭 (最新) から順に返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Vec<String>` - 復号済み履歴本文の一覧 (先頭が最新)
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn history_texts(&self) -> Vec<String> {
        (0..self.history_len())
            .filter_map(|i| self.get_history_entry(i).map(|s| s.to_string()))
            .collect()
    }

    /// 履歴をクリアする
    ///
    /// # Arguments
    /// * `self` - 自身
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
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `usize` - 数値
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

    /// 連続記録をすべて復号してキュー先頭から順に返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Vec<String>` - 復号済み連続記録本文の一覧 (先頭がキュー先頭)
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn continuous_record_texts(&self) -> Vec<String> {
        (0..self.continuous_record_len())
            .filter_map(|i| self.get_continuous_record_entry(i).map(|s| s.to_string()))
            .collect()
    }

    /// 連続記録をクリアする
    ///
    /// # Arguments
    /// * `self` - 自身
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
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<SecretString>` - 値がある場合は `Some`、ない場合は `None`
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

    /// 取り出した連続記録を元のインデックスへ戻す
    ///
    /// 空文字やトリム後に空になる文字列は無視される
    ///
    /// # Arguments
    /// * `index` - 戻す位置 (0 が先頭)
    /// * `text` - 戻すクリップボード本文
    /// * `limit` - 保持する最大件数
    ///
    /// # Returns
    /// * `bool` - 戻された場合は `true`
    pub fn restore_continuous_record_at(
        &self,
        index: usize,
        text: impl AsRef<str>,
        limit: usize,
    ) -> bool {
        let text = text.as_ref();
        if text.trim().is_empty() {
            return false;
        }
        self.continuous_record_store
            .lock_ignore_poison()
            .restore_at(index, text, limit);
        true
    }

    /// 連続記録キュー末尾 (直近の記録) を取り消す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `Option<SecretString>` - 値がある場合は `Some`、ない場合は `None`
    pub fn undo_last_continuous_record(&self) -> Option<SecretString> {
        self.continuous_record_store.lock_ignore_poison().pop_back()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::ContentFingerprint;
    use crate::tray::state::ProcessedState;

    fn session() -> ClipboardSessionState {
        ClipboardSessionState::new().expect("テスト用クリップボードセッションの生成に失敗")
    }

    /// 加工成功時に書き戻し本文と観測済み本文が更新されるようにすること
    #[test]
    fn record_processing_success_updates_fingerprints() {
        let session = session();
        session.record_processing_success("processed");

        let ps = session.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("processed"));
        assert!(ps.matches_last_written("processed"));
    }

    /// 観測のみの場合は `last_written` を変更しないようにすること
    #[test]
    fn record_clipboard_observed_preserves_last_written() {
        let session = session();
        session.with_processed_state(|ps| {
            ps.last_written = Some(ContentFingerprint::from_text("written"));
            ps.last_seen = ContentFingerprint::from_text("old");
        });

        session.record_clipboard_observed("observed");

        let ps = session.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("observed"));
        assert!(ps.matches_last_written("written"));
    }

    /// 履歴復元など外部設定時は書き戻しフラグを1回分のエコーとして記録すること
    #[test]
    fn record_clipboard_set_updates_both_fingerprints() {
        let session = session();
        session.with_processed_state(|ps| {
            ps.last_written = Some(ContentFingerprint::from_text("written"));
            ps.last_seen = ContentFingerprint::from_text("old");
        });

        session.record_clipboard_set("restored");

        let ps = session.with_processed_state(|s| s.clone());
        assert!(ps.matches_last_seen("restored"));
        assert!(ps.matches_last_written("restored"));
    }

    /// 加工取り消し用テキストの記録と取得ができるようにすること
    #[test]
    fn undo_source_record_and_take() {
        let session = session();

        assert!(session.take_undo_source().is_none());

        session.record_undo_source("original", "processed");
        assert_eq!(
            session.take_undo_source().as_ref().map(|s| s.as_str()),
            Some("original")
        );
        assert!(session.take_undo_source().is_none());
    }

    /// 現在のクリップボードが加工直後のままなら取り消せること
    #[test]
    fn undo_entry_is_ready_when_clipboard_is_unchanged() {
        let session = session();
        session.record_undo_source("original", "processed");

        let UndoLookup::Ready(entry) = session.take_undo_entry_if_current(Some("processed")) else {
            panic!("取り消せる状態であること");
        };
        assert_eq!(entry.source.as_str(), "original");
    }

    /// 加工後に別の内容がコピーされていたら取り消しを拒否し、記録を破棄すること
    #[test]
    fn undo_entry_is_stale_when_clipboard_changed() {
        let session = session();
        session.record_undo_source("original", "processed");

        assert!(matches!(
            session.take_undo_entry_if_current(Some("copied-later")),
            UndoLookup::Stale
        ));
        assert!(matches!(
            session.take_undo_entry_if_current(Some("processed")),
            UndoLookup::Empty
        ));
    }

    /// テキストを読めない場合、テキスト加工の取り消しは拒否すること
    #[test]
    fn undo_entry_is_stale_when_text_unreadable() {
        let session = session();
        session.record_undo_source("original", "processed");

        assert!(matches!(
            session.take_undo_entry_if_current(None),
            UndoLookup::Stale
        ));
    }

    /// 画像加工の取り消しはテキストを読めない状態を加工直後とみなすこと
    ///
    /// テキストを読める場合は加工前テキストとの一致を要求する
    #[test]
    fn image_undo_entry_accepts_unreadable_text_only_when_unchanged() {
        let session = session();

        session.record_image_undo_source("original");
        assert!(matches!(
            session.take_undo_entry_if_current(None),
            UndoLookup::Ready(_)
        ));

        session.record_image_undo_source("original");
        assert!(matches!(
            session.take_undo_entry_if_current(Some("copied-later")),
            UndoLookup::Stale
        ));
    }

    /// `with_processed_state` で二重加工防止状態を更新できるようにすること
    #[test]
    fn with_processed_state_updates_last_seen() {
        let session = session();
        let ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("hello"),
            ..Default::default()
        };
        session.with_processed_state(|s| *s = ps);
        assert!(session.with_processed_state(|s| s.matches_last_seen("hello")));
    }
}
