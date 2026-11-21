//! アプリ更新: ローカル HTTP bridge 経由の確認 → ダウンロード検証 → 適用

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use clip_refiner::test_helpers::{
    MockReleaseBridge, UpdateApplyOutcome, UpdateCheckOutcome, UpdatePackageKind,
    apply_update_for_test, build_portable_zip_fixture, check_update_at, version_api_json,
};

/// ケースごとに一意なバージョン末尾を割り当てる
static CASE_SEQ: AtomicU64 = AtomicU64::new(1);

// ======================================================================
// ヘルパー
// ======================================================================
/// 一意な更新バージョン文字列を返す
fn unique_version() -> String {
    let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
    format!("99.0.{n}")
}

/// 現在アーキ向けポータブル ZIP アセット名を返す
fn portable_asset_name(version: &str) -> String {
    format!(
        "clip-refiner-{version}-{}-pc-windows-msvc-portable.zip",
        std::env::consts::ARCH
    )
}

/// 現在アーキ向け MSI アセット名を返す
fn msi_asset_name(version: &str) -> String {
    format!("clip-refiner-{version}-{}.msi", std::env::consts::ARCH)
}

/// 更新作業ディレクトリを掃除する
fn cleanup_update_work_dir(version: &str) {
    let dir = std::env::temp_dir()
        .join("clip-refiner-update")
        .join(version);
    let _ = fs::remove_dir_all(dir);
}

// ======================================================================
// テスト
// ======================================================================

/// ローカル bridge で更新確認からポータブル ZIP 展開まで通ること
#[test]
fn update_flow_check_and_apply_portable_zip() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let marker = b"portable-flow-ok";
    let (zip_bytes, digest, size) = build_portable_zip_fixture(marker);

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    bridge.set_asset(&tag, &asset_name, zip_bytes);

    let check = check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    );
    let update = match check {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    };
    assert_eq!(update.latest_version, version);
    assert_eq!(update.asset_name, asset_name);
    assert_eq!(update.size, size);
    assert_eq!(update.sha256, digest);

    let apply = apply_update_for_test(&update, bridge.base_url());
    let path = match apply {
        UpdateApplyOutcome::PortableExtracted {
            path,
            helper_launched,
        } => {
            assert!(
                !helper_launched,
                "テスト適用では差し替えヘルパーを起動しない"
            );
            PathBuf::from(path)
        }
        other => panic!("想定外の適用結果: {other:?}"),
    };
    let content = fs::read_to_string(path.join("PORTABLE.txt")).expect("PORTABLE.txt");
    assert_eq!(content.as_bytes(), marker);

    cleanup_update_work_dir(&version);
}

/// 同一バージョンは `UpToDate` になること
#[test]
fn update_flow_reports_up_to_date() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, digest, size) = build_portable_zip_fixture(b"up-to-date");

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    bridge.set_asset(&tag, &asset_name, zip_bytes);

    let outcome = check_update_at(
        bridge.base_url(),
        &version,
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    );
    match outcome {
        UpdateCheckOutcome::UpToDate {
            current_version,
            latest_version,
        } => {
            assert_eq!(current_version, version);
            assert_eq!(latest_version, version);
        }
        other => panic!("想定外: {other:?}"),
    }
}

/// digest 欠落時は確認がエラーになること
#[test]
fn update_flow_errors_when_digest_missing() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, size) = build_portable_zip_fixture(b"no-digest");

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(&tag, &asset_name, size, None));
    bridge.set_asset(&tag, &asset_name, zip_bytes);

    let outcome = check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    );
    match outcome {
        UpdateCheckOutcome::Error { message } => {
            assert!(message.contains("digest"));
        }
        other => panic!("想定外: {other:?}"),
    }
}

/// SHA-256 不一致時は適用せずエラーになること
#[test]
fn update_flow_rejects_sha256_mismatch_on_apply() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, size) = build_portable_zip_fixture(b"bad-hash");
    let wrong_digest = "0".repeat(64);

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{wrong_digest}")),
    ));
    bridge.set_asset(&tag, &asset_name, zip_bytes);

    let update = match check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    ) {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    };

    let apply = apply_update_for_test(&update, bridge.base_url());
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains("SHA-256"));
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }

    let archive_path = std::env::temp_dir()
        .join("clip-refiner-update")
        .join(&version)
        .join(&asset_name);
    assert!(
        !archive_path.exists(),
        "検証失敗時はダウンロードファイルを残さない"
    );

    cleanup_update_work_dir(&version);
}

/// サイズ不一致時は適用せずエラーになること
#[test]
fn update_flow_rejects_size_mismatch_on_apply() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, digest, size) = build_portable_zip_fixture(b"bad-size");
    assert!(size > 1);

    let bridge = MockReleaseBridge::start();
    // API 上の size だけずらし、本文と digest は実ファイルに合わせる
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size + 1,
        Some(&format!("sha256:{digest}")),
    ));
    bridge.set_asset(&tag, &asset_name, zip_bytes);

    let update = match check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    ) {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    };
    assert_eq!(update.size, size + 1);

    let apply = apply_update_for_test(&update, bridge.base_url());
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains("サイズ"));
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }

    cleanup_update_work_dir(&version);
}

/// 応答本文が宣言サイズを超える場合は途中で打ち切り適用しないこと
#[test]
fn update_flow_aborts_download_when_body_exceeds_declared_size() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, digest, size) = build_portable_zip_fixture(b"oversized-body");
    assert!(size > 1);

    let mut oversized = zip_bytes;
    oversized.extend_from_slice(&[0u8; 4096]);

    let bridge = MockReleaseBridge::start();
    // API の size / digest は正当な ZIP 側に合わせ、HTTP 本文だけ大きくする
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    bridge.set_asset(&tag, &asset_name, oversized);

    let update = match check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    ) {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    };
    assert_eq!(update.size, size);

    let apply = apply_update_for_test(&update, bridge.base_url());
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains("サイズ"));
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }

    let archive_path = std::env::temp_dir()
        .join("clip-refiner-update")
        .join(&version)
        .join(&asset_name);
    assert!(
        !archive_path.exists(),
        "打ち切り時はダウンロードファイルを残さない"
    );

    cleanup_update_work_dir(&version);
}

/// version API が 404 のとき確認がエラーになること
#[test]
fn update_flow_errors_when_version_api_missing() {
    let bridge = MockReleaseBridge::start();
    bridge.clear_version_json();

    let outcome = check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    );
    match outcome {
        UpdateCheckOutcome::Error { message } => {
            assert!(message.contains("更新情報") || message.contains("HTTP"));
        }
        other => panic!("想定外: {other:?}"),
    }
}

/// MSI アセットもダウンロード検証まで通ること (`msiexec` は起動しない)
#[test]
fn update_flow_check_and_verify_msi_without_launch() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = msi_asset_name(&version);
    let payload = b"fake-msi-payload";
    let digest = clip_refiner::test_helpers::sha256_hex(payload);
    let size = payload.len() as u64;

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    bridge.set_asset(&tag, &asset_name, payload.to_vec());

    let update = match check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::Msi,
        std::env::consts::ARCH,
    ) {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    };

    let apply = apply_update_for_test(&update, bridge.base_url());
    assert!(
        matches!(apply, UpdateApplyOutcome::MsiStarted),
        "想定外: {apply:?}"
    );

    let archive_path = std::env::temp_dir()
        .join("clip-refiner-update")
        .join(&version)
        .join(&asset_name);
    assert_eq!(fs::read(&archive_path).expect("msi bytes"), payload);

    cleanup_update_work_dir(&version);
}
