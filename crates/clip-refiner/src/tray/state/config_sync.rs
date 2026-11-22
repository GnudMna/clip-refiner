use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use super::lock_ext::LockExt;
use crate::config::{ConfigFileFingerprint, disk_config_content_fingerprint};

// ======================================================================
// 設定ディスク同期
// ======================================================================
/// 設定ファイルのディスク同期状態
///
/// 変更判定は更新時刻ではなく本文指紋を使う。自身の保存中は外部変更とみなさない
pub(crate) struct ConfigSyncState {
    persist_config: bool,
    fingerprint: Mutex<Option<ConfigFileFingerprint>>,
    /// 再読み込みに失敗した本文の指紋 (同じ本文で失敗通知を繰り返さないために保持する)
    failed_fingerprint: Mutex<Option<ConfigFileFingerprint>>,
    own_write: AtomicBool,
    /// 設定の複製からディスクへの書き込み完了までを直列化するロック
    ///
    /// 設定本体のロックを保存中に持たない代わりに、古い複製が新しい複製を上書きしないようにする
    save_lock: Mutex<()>,
    /// 登録クリップの鍵を復号できない旨を通知済みか (通知の繰り返しを防ぐ)
    key_unavailable_notified: AtomicBool,
}

impl ConfigSyncState {
    /// ディスク保存を有効にした同期状態を生成する
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    pub fn new() -> Self {
        let fingerprint = disk_config_content_fingerprint().ok().flatten();
        Self {
            persist_config: true,
            fingerprint: Mutex::new(fingerprint),
            failed_fingerprint: Mutex::new(None),
            own_write: AtomicBool::new(false),
            save_lock: Mutex::new(()),
            key_unavailable_notified: AtomicBool::new(false),
        }
    }

    /// テスト用にディスク保存を無効化した同期状態を生成する
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn new_for_test() -> Self {
        Self {
            persist_config: false,
            fingerprint: Mutex::new(None),
            failed_fingerprint: Mutex::new(None),
            own_write: AtomicBool::new(false),
            save_lock: Mutex::new(()),
            key_unavailable_notified: AtomicBool::new(false),
        }
    }

    /// テスト用にディスク保存を有効化した同期状態を生成する
    ///
    /// `with_temp_config_dir` 内でのみ使うこと
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn new_for_test_with_persistence() -> Self {
        Self::new()
    }

    /// 設定のディスク保存が有効かどうかを返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - ディスク保存が有効な場合は `true`
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn is_persistence_enabled(&self) -> bool {
        self.persist_config
    }

    /// ディスク上の設定ファイルと同期済みの内容指紋を記録する
    ///
    /// 記録時点のディスクを読み直すため、直前の読み込み・保存との間に入った外部編集も
    /// 同期済みとして扱う。読み込みや保存の結果の指紋が分かる場合は `record_synced` を使う
    ///
    /// # Arguments
    /// * `self` - 自身
    #[cfg(test)]
    pub fn record_disk_sync(&self) {
        if let Ok(fingerprint) = disk_config_content_fingerprint() {
            self.record_synced(fingerprint);
        }
    }

    /// 実際にメモリへ反映した本文、または書き込んだ本文の指紋を同期済みとして記録する
    ///
    /// # Arguments
    /// * `fingerprint` - 反映・書き込みした本文の指紋。取得できなかった場合は `None`
    pub fn record_synced(&self, fingerprint: Option<ConfigFileFingerprint>) {
        *self.fingerprint.lock_ignore_poison() = fingerprint;
        *self.failed_fingerprint.lock_ignore_poison() = None;
    }

    /// 再読み込みに失敗した本文の指紋を記録する
    ///
    /// 同じ本文では外部変更とみなさず、失敗通知の繰り返しを防ぐ。本文が変われば再び検知する
    ///
    /// # Arguments
    /// * `fingerprint` - 再読み込みを試みた時点の本文指紋
    pub fn record_reload_failure(&self, fingerprint: Option<ConfigFileFingerprint>) {
        *self.failed_fingerprint.lock_ignore_poison() = fingerprint;
    }

    /// 外部編集による設定ファイルの変更を検知する
    ///
    /// 自身の保存中、本文指紋が最終同期または再読み込み失敗時と一致する場合、
    /// ファイルが無いか読み取りに失敗した場合は `false` を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - ディスク上の本文が最終同期とも再読み込み失敗時とも異なる場合は `true`
    pub fn has_external_change(&self) -> bool {
        if self.own_write.load(Ordering::Acquire) {
            return false;
        }
        let Ok(current) = disk_config_content_fingerprint() else {
            return false;
        };
        if current.is_some() && current == *self.failed_fingerprint.lock_ignore_poison() {
            return false;
        }
        match (*self.fingerprint.lock_ignore_poison(), current) {
            (Some(known), Some(file)) => known != file,
            (None, Some(_)) => true,
            (Some(_) | None, None) => false,
        }
    }

    /// 保存の直列化ロックを取得する
    ///
    /// 設定の複製を取る前に取得し、書き込み完了まで保持する
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `MutexGuard<'_, ()>` - 保存の直列化ロックのガード
    pub fn lock_save(&self) -> MutexGuard<'_, ()> {
        self.save_lock.lock_ignore_poison()
    }

    /// 自身の保存を開始し、完了まで外部変更検知を抑制する
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn begin_own_write(&self) {
        self.own_write.store(true, Ordering::Release);
    }

    /// 自身の保存を終了し、外部変更検知を再開する
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn end_own_write(&self) {
        self.own_write.store(false, Ordering::Release);
    }

    /// 登録クリップの鍵を復号できない旨の通知権を取得する
    ///
    /// 保存のたびに通知が繰り返されないよう、プロセス内で最初の 1 回だけ `true` を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - 初めて呼ばれた場合は `true`
    pub fn claim_key_unavailable_notice(&self) -> bool {
        !self.key_unavailable_notified.swap(true, Ordering::AcqRel)
    }

    /// ディスクへの設定保存が有効かどうか
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - ディスク保存が有効な場合は `true`
    pub fn should_persist(&self) -> bool {
        self.persist_config
    }
}

impl Default for ConfigSyncState {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::test_helpers::with_temp_config_dir;

    /// 鍵の通知権は最初の 1 回だけ取得できること
    #[test]
    fn key_unavailable_notice_is_claimed_once() {
        let sync = ConfigSyncState::new_for_test();
        assert!(sync.claim_key_unavailable_notice());
        assert!(!sync.claim_key_unavailable_notice());
    }

    /// テスト用生成ではディスク保存が無効となるようにすること
    #[test]
    fn new_for_test_disables_persistence() {
        let sync = ConfigSyncState::new_for_test();
        assert!(!sync.should_persist());
        assert!(!sync.is_persistence_enabled());
    }

    /// 自身の保存中は外部変更を検知しないようにすること
    #[test]
    fn has_external_change_false_during_own_write() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test_with_persistence();
            sync.record_disk_sync();
            sync.begin_own_write();

            let changed = AppConfig {
                interval_ms: 2500,
                ..AppConfig::default()
            };
            changed.save().expect("external-looking save");

            assert!(!sync.has_external_change());
            sync.end_own_write();
            assert!(sync.has_external_change());
        });
    }

    /// 設定ファイルが無いときは外部変更なしと判定すること
    #[test]
    fn has_external_change_false_when_config_missing() {
        with_temp_config_dir(|| {
            let sync = ConfigSyncState::new_for_test();
            assert!(!sync.has_external_change());
        });
    }

    /// 同期指紋未記録かつファイルがあるときは外部変更と判定すること
    #[test]
    fn has_external_change_true_when_fingerprint_unknown() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test();
            assert!(sync.has_external_change());
        });
    }

    /// 本文が変わったときは待ちなしで外部変更と判定すること
    #[test]
    fn has_external_change_true_when_content_differs() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test_with_persistence();
            sync.record_disk_sync();
            assert!(!sync.has_external_change());

            let changed = AppConfig {
                interval_ms: 2500,
                ..AppConfig::default()
            };
            changed.save().expect("config resave");

            assert!(sync.has_external_change());
        });
    }

    /// 再読み込みに失敗した本文は外部変更と判定せず、本文が変われば再び判定すること
    #[test]
    fn has_external_change_false_for_failed_content_until_it_changes() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test_with_persistence();
            sync.record_disk_sync();

            let path = crate::config::get_config_dir()
                .expect("dir")
                .join("config.toml");
            std::fs::write(&path, "mode = [[invalid").expect("壊れた設定");
            assert!(sync.has_external_change());

            sync.record_reload_failure(disk_config_content_fingerprint().expect("fingerprint"));
            assert!(!sync.has_external_change());

            // 別の壊れた内容へ変わった場合は再び検知する
            std::fs::write(&path, "mode = [[invalid2").expect("別の壊れた設定");
            assert!(sync.has_external_change());

            // 同期済みの内容へ戻れば失敗の記録は消える
            std::fs::write(&path, "mode = [[invalid").expect("元の壊れた設定");
            sync.record_disk_sync();
            assert!(!sync.has_external_change());
        });
    }

    /// 同一本文の再書き込みは外部変更と判定しないようにすること
    #[test]
    fn has_external_change_false_when_bytes_unchanged() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test_with_persistence();
            sync.record_disk_sync();

            let path = crate::config::get_config_dir()
                .expect("dir")
                .join("config.toml");
            let bytes = std::fs::read(&path).expect("read");
            std::fs::write(&path, bytes).expect("identical rewrite");

            assert!(!sync.has_external_change());
        });
    }
}
