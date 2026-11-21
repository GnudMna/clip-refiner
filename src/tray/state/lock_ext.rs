use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

// ======================================================================
// ロック拡張
// ======================================================================
/// `Mutex` のポイズニング(パニックによる汚染)を無視して強制的にロックを取得するための拡張
pub trait LockExt<T> {
    /// ロックを取得する。ポイズニングされている場合は汚染された状態のままデータを取得する。
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// `RwLock` のポイズニングを無視して強制的にロックを取得するための拡張
pub trait RwLockExt<T> {
    /// 読み取りロックを取得する
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T>;
    /// 書き込みロックを取得する
    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T> {
        self.read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T> {
        self.write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
