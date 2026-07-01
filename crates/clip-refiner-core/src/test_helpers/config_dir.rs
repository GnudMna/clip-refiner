use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::{clear_test_config_dir, set_test_config_dir};

// ======================================================================
// テスト用設定ディレクトリ
// ======================================================================
/// 一時的な設定ディレクトリでクロージャを実行する
///
/// 並列テストでも衝突しないよう、スレッドごとに独立したディレクトリを使う
pub fn with_temp_config_dir<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("clip-refiner-test-{}-{}", std::process::id(), id));
    fs::create_dir_all(&dir).expect("テスト用設定ディレクトリの作成に失敗");

    set_test_config_dir(dir.clone());
    let result = f();
    clear_test_config_dir();
    let _ = fs::remove_dir_all(dir);
    result
}
