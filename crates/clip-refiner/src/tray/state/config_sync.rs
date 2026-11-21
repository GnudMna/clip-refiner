use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use super::lock_ext::LockExt;

// ======================================================================
// 設定ディスク同期
// ======================================================================
/// 設定ファイルのディスク同期状態と保存グレース期間
pub(crate) struct ConfigSyncState {
    persist_config: bool,
    disk_mtime: Mutex<Option<SystemTime>>,
    save_grace_until: Mutex<Option<Instant>>,
}

impl ConfigSyncState {
    /// ディスク保存を有効にした同期状態を生成する
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    pub fn new() -> Self {
        let disk_mtime = crate::config::disk_config_modified_time().ok().flatten();
        Self {
            persist_config: true,
            disk_mtime: Mutex::new(disk_mtime),
            save_grace_until: Mutex::new(None),
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
            disk_mtime: Mutex::new(None),
            save_grace_until: Mutex::new(None),
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

    /// ディスク上の設定ファイルと同期済みの更新時刻を記録する
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn record_disk_sync(&self) {
        if let Ok(mtime) = crate::config::disk_config_modified_time() {
            *self.disk_mtime.lock_ignore_poison() = mtime;
        }
    }

    /// 外部編集による設定ファイルの変更を検知する
    ///
    /// アプリ自身の保存直後は同期済み時刻と一致するため `false` を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - ディスク上の設定が最終同期時刻より新しい場合は `true`
    pub fn has_external_change(&self) -> bool {
        if self.is_save_grace_active() {
            return false;
        }
        let Ok(Some(file_mtime)) = crate::config::disk_config_modified_time() else {
            return false;
        };
        match *self.disk_mtime.lock_ignore_poison() {
            Some(known) => file_mtime > known,
            None => true,
        }
    }

    /// 自身の保存直後に外部変更検知を一時抑制する
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn begin_save_grace(&self) {
        *self.save_grace_until.lock_ignore_poison() = Some(Instant::now() + Duration::from_secs(2));
    }

    /// 自身の保存直後のグレース期間中かどうか
    fn is_save_grace_active(&self) -> bool {
        self.save_grace_until
            .lock_ignore_poison()
            .is_some_and(|until| Instant::now() < until)
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

    /// 保存グレース期間中は外部変更を検知しないようにすること
    #[test]
    fn has_external_change_false_during_save_grace() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test_with_persistence();
            sync.record_disk_sync();
            sync.begin_save_grace();

            assert!(!sync.has_external_change());
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

    /// 同期時刻未記録かつファイルがあるときは外部変更と判定すること
    #[test]
    fn has_external_change_true_when_mtime_unknown() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test();
            assert!(sync.has_external_change());
        });
    }

    /// ディスクの更新時刻が新しいときは外部変更と判定すること
    #[test]
    fn has_external_change_true_when_disk_newer() {
        with_temp_config_dir(|| {
            AppConfig::default().save().expect("config save");
            let sync = ConfigSyncState::new_for_test_with_persistence();
            sync.record_disk_sync();
            assert!(!sync.has_external_change());

            // Windows でも確実に新しい mtime になるよう少し待つ
            std::thread::sleep(Duration::from_millis(1100));
            AppConfig::default().save().expect("config resave");

            assert!(sync.has_external_change());
        });
    }
}
