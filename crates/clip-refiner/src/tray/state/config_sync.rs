use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

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
    own_write: AtomicBool,
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
            own_write: AtomicBool::new(false),
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
            own_write: AtomicBool::new(false),
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
    /// # Arguments
    /// * `self` - 自身
    pub fn record_disk_sync(&self) {
        if let Ok(fingerprint) = disk_config_content_fingerprint() {
            *self.fingerprint.lock_ignore_poison() = fingerprint;
        }
    }

    /// 外部編集による設定ファイルの変更を検知する
    ///
    /// 自身の保存中、本文指紋が最終同期と一致する場合、ファイルが無いか読み取りに失敗した場合は `false` を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - ディスク上の本文が最終同期と異なる場合は `true`
    pub fn has_external_change(&self) -> bool {
        if self.own_write.load(Ordering::Acquire) {
            return false;
        }
        let Ok(current) = disk_config_content_fingerprint() else {
            return false;
        };
        match (*self.fingerprint.lock_ignore_poison(), current) {
            (Some(known), Some(file)) => known != file,
            (None, Some(_)) => true,
            (Some(_) | None, None) => false,
        }
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
