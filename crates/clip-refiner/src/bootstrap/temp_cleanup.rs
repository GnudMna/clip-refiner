//! 一時フォルダ (`%TEMP%`) に残った、このアプリの作業ディレクトリの掃除
//!
//! 更新の作業ディレクトリと、`WebView2` の退避用データディレクトリのうち、
//! 保持時間を過ぎたものを常駐の起動時に削除する

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::consts;
use crate::platform::update::UPDATE_WORK_DIR_NAME;

// ======================================================================
// 定数
// ======================================================================
/// 作業ディレクトリの保持時間 (秒)
///
/// 更新の適用中に起動した別プロセスの作業ディレクトリを消さないよう、1 日残す
const RETENTION_SECS: u64 = 24 * 60 * 60;

/// `WebView2` データディレクトリ名に含まれる識別部分
const WEBVIEW_DIR_MARKER: &str = "-WebView2";

/// `WebView2` の初期化失敗時に使う退避用データディレクトリ名の接頭辞
const FALLBACK_DIR_PREFIX: &str = "pid-";

// ======================================================================
// パブリック関数
// ======================================================================
/// 保持時間を過ぎた作業ディレクトリを一時フォルダから削除する
///
/// 対象は `clip-refiner-update` 直下の版ごとのフォルダと、
/// `ClipRefiner-*-WebView2*` 直下の `pid-*` フォルダ。
/// 削除できないものは残し、次回の起動時に再試行する
pub(super) fn remove_stale_temp_dirs() {
    remove_stale_temp_dirs_in(
        &std::env::temp_dir(),
        Duration::from_secs(RETENTION_SECS),
        SystemTime::now(),
    );
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 指定した一時フォルダ直下の作業ディレクトリを掃除する
fn remove_stale_temp_dirs_in(temp: &Path, retention: Duration, now: SystemTime) {
    remove_stale_children(&temp.join(UPDATE_WORK_DIR_NAME), |_| true, retention, now);

    let Ok(entries) = fs::read_dir(temp) else {
        return;
    };
    for entry in entries.flatten() {
        let is_webview_dir = entry.file_name().to_str().is_some_and(|name| {
            name.starts_with(&format!("{}-", consts::APP_NAME)) && name.contains(WEBVIEW_DIR_MARKER)
        });
        if is_webview_dir {
            remove_stale_children(
                &entry.path(),
                |name| name.starts_with(FALLBACK_DIR_PREFIX),
                retention,
                now,
            );
        }
    }
}

/// `dir` 直下のフォルダのうち、名前が `is_target` を満たし保持時間を過ぎたものを削除する
///
/// シンボリックリンクは辿らず対象外にする。`dir` が空になった場合は `dir` も削除する
fn remove_stale_children(
    dir: &Path,
    is_target: impl Fn(&str) -> bool,
    retention: Duration,
    now: SystemTime,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let targeted = entry.file_name().to_str().is_some_and(&is_target);
        let is_real_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if !targeted || !is_real_dir {
            continue;
        }

        let age = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok());
        if age.is_some_and(|age| age >= retention) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
    // 空でなければ失敗して残る
    let _ = fs::remove_dir(dir);
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_hours(24);

    fn unique_temp(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("clip-refiner-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("一時フォルダの作成に失敗");
        dir
    }

    /// 保持時間を過ぎた更新の作業ディレクトリは削除し、新しいものは残すこと
    #[test]
    fn removes_only_expired_update_work_dirs() {
        let temp = unique_temp("temp-cleanup-update");
        let version_dir = temp.join(UPDATE_WORK_DIR_NAME).join("1.2.3");
        fs::create_dir_all(version_dir.join("extracted")).expect("作業フォルダの作成に失敗");
        fs::write(version_dir.join("app.msi"), b"msi").expect("ファイルの作成に失敗");

        // 保持時間内の時刻では残る
        remove_stale_temp_dirs_in(&temp, DAY, SystemTime::now());
        assert!(version_dir.exists());

        // 保持時間を過ぎた時刻では、版のフォルダも空になった親も消える
        remove_stale_temp_dirs_in(&temp, DAY, SystemTime::now() + DAY * 2);
        assert!(!version_dir.exists());
        assert!(!temp.join(UPDATE_WORK_DIR_NAME).exists());

        let _ = fs::remove_dir_all(&temp);
    }

    /// `WebView2` の `pid-*` だけを削除し、通常のデータディレクトリと無関係なフォルダは残すこと
    #[test]
    fn removes_only_fallback_webview_dirs() {
        let temp = unique_temp("temp-cleanup-webview");
        let base = temp.join(format!("{}-Settings-WebView2", consts::APP_NAME));
        let e2e_base = temp.join(format!(
            "{}-QuickSelector-WebView2-e2e-9222",
            consts::APP_NAME
        ));
        let fallback = base.join("pid-1234");
        let e2e_fallback = e2e_base.join("pid-5678");
        let profile = base.join("EBWebView");
        let unrelated = temp.join("Other-WebView2").join("pid-1");
        for dir in [&fallback, &e2e_fallback, &profile, &unrelated] {
            fs::create_dir_all(dir).expect("フォルダの作成に失敗");
        }

        remove_stale_temp_dirs_in(&temp, DAY, SystemTime::now() + DAY * 2);

        assert!(!fallback.exists());
        assert!(!e2e_fallback.exists());
        // 通常のデータディレクトリと、このアプリのものではないフォルダは対象外
        assert!(profile.exists());
        assert!(unrelated.exists());

        let _ = fs::remove_dir_all(&temp);
    }

    /// 一時フォルダが無い場合や、対象のフォルダが無い場合は何もしないこと
    #[test]
    fn ignores_missing_dirs() {
        let temp = std::env::temp_dir().join("clip-refiner-temp-cleanup-missing-never-created");
        remove_stale_temp_dirs_in(&temp, DAY, SystemTime::now() + DAY * 2);
        assert!(!temp.exists());
    }
}
