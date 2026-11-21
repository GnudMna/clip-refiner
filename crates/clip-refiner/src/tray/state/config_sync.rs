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
    pub fn new() -> Self {
        let disk_mtime = crate::config::disk_config_modified_time().ok().flatten();
        Self {
            persist_config: true,
            disk_mtime: Mutex::new(disk_mtime),
            save_grace_until: Mutex::new(None),
        }
    }

    /// テスト用にディスク保存を無効化した同期状態を生成する
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn new_for_test() -> Self {
        Self {
            persist_config: false,
            disk_mtime: Mutex::new(None),
            save_grace_until: Mutex::new(None),
        }
    }

    /// 設定のディスク保存が有効かどうかを返す
    #[cfg(test)]
    pub fn is_persistence_enabled(&self) -> bool {
        self.persist_config
    }

    /// ディスク上の設定ファイルと同期済みの更新時刻を記録する
    pub fn record_disk_sync(&self) {
        if let Ok(mtime) = crate::config::disk_config_modified_time() {
            *self.disk_mtime.lock_ignore_poison() = mtime;
        }
    }

    /// 外部編集による設定ファイルの変更を検知する
    ///
    /// アプリ自身の保存直後は同期済み時刻と一致するため `false` を返す
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
    pub fn should_persist(&self) -> bool {
        self.persist_config
    }
}

impl Default for ConfigSyncState {
    fn default() -> Self {
        Self::new()
    }
}
