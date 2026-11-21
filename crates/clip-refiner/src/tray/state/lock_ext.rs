use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

// ======================================================================
// ロック拡張
// ======================================================================
/// `Mutex` / `RwLock` のポイズニングを無視してロックを取得するための拡張
///
/// トレイ常駐アプリでは、保持者スレッドのパニックでロックが汚染されても
/// プロセス全体を終了させず UI と監視を継続する方針を取る
/// ポイズン発生時はエラーログを出し、汚染されたガードを返して処理を続行する
pub trait LockExt<T> {
    /// ロックを取得する
    ///
    /// ポイズニングされている場合は汚染された状態のままデータを取得する
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poison) => {
                crate::log_error!(
                    "Mutex がポイズン状態になっています。データを復旧して処理を継続します"
                );
                poison.into_inner()
            }
        }
    }
}

/// `RwLock` のポイズニングを無視して強制的にロックを取得するための拡張
pub trait RwLockExt<T> {
    /// 読み取りロックを取得する
    ///
    /// ポイズニングされている場合は汚染された状態のままデータを取得する
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T>;
    /// 書き込みロックを取得する
    ///
    /// ポイズニングされている場合は汚染された状態のままデータを取得する
    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T> {
        match self.read() {
            Ok(guard) => guard,
            Err(poison) => {
                crate::log_error!(
                    "RwLock の読み取りロックがポイズン状態です。データを復旧して処理を継続します"
                );
                poison.into_inner()
            }
        }
    }

    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T> {
        match self.write() {
            Ok(guard) => guard,
            Err(poison) => {
                crate::log_error!(
                    "RwLock の書き込みロックがポイズン状態です。データを復旧して処理を継続します"
                );
                poison.into_inner()
            }
        }
    }
}
