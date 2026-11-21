//! アプリ更新: ローカル HTTP bridge 経由の確認 → ダウンロード検証 → 適用

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use clip_refiner::test_helpers::{
    AvailableUpdate, MockReleaseBridge, UpdateApplyOutcome, UpdateCheckOutcome, UpdatePackageKind,
    apply_update_for_test, apply_update_for_test_with_key, build_portable_zip_fixture,
    check_update_at, sha256_hex, update_signature_asset_name, update_signature_trusted_comment,
    version_api_json,
};

use minisign::KeyPair;

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

/// ダウンロード済みアセットの保存先を返す
fn downloaded_archive_path(version: &str, asset_name: &str) -> PathBuf {
    std::env::temp_dir()
        .join("clip-refiner-update")
        .join(version)
        .join(asset_name)
}

/// テスト全体で共有するリリース署名鍵ペアを返す
fn signing_key_pair() -> &'static KeyPair {
    static KEY_PAIR: OnceLock<KeyPair> = OnceLock::new();
    KEY_PAIR.get_or_init(|| KeyPair::generate_unencrypted_keypair().expect("keypair"))
}

/// 署名検証で信頼する公開鍵 (Base64) を返す
fn trusted_key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| signing_key_pair().pk.to_base64())
}

/// 指定鍵・信頼コメントで minisign 署名ファイルの内容を作る
fn sign_with(key_pair: &KeyPair, body: &[u8], trusted_comment: &str) -> Vec<u8> {
    minisign::sign(
        Some(&key_pair.pk),
        &key_pair.sk,
        Cursor::new(body),
        Some(trusted_comment),
        None,
    )
    .expect("sign")
    .into_string()
    .into_bytes()
}

/// リリース手順と同じ形式の署名をモック bridge へ載せる
fn publish_signature(
    bridge: &MockReleaseBridge,
    tag: &str,
    version: &str,
    asset_name: &str,
    body: &[u8],
) {
    let comment = update_signature_trusted_comment(version, asset_name);
    bridge.set_asset(
        tag,
        &update_signature_asset_name(asset_name),
        sign_with(signing_key_pair(), body, &comment),
    );
}

/// 正しい digest を持つポータブル ZIP を載せ、更新確認まで進める
///
/// 署名は呼び出し側で用意する
fn check_portable_update(
    bridge: &MockReleaseBridge,
    version: &str,
    asset_name: &str,
    zip_bytes: Vec<u8>,
) -> AvailableUpdate {
    let tag = format!("v{version}");
    let digest = sha256_hex(&zip_bytes);
    bridge.set_version_json(version_api_json(
        &tag,
        asset_name,
        zip_bytes.len() as u64,
        Some(&format!("sha256:{digest}")),
    ));
    bridge.set_asset(&tag, asset_name, zip_bytes);

    match check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    ) {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    }
}

/// 適用結果がエラーで、メッセージに `needle` を含むことを確かめる
fn assert_apply_error(apply: UpdateApplyOutcome, needle: &str) {
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains(needle), "想定外のメッセージ: {message}");
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }
}

// ======================================================================
// テスト
// ======================================================================

/// ローカル bridge で更新確認からポータブル ZIP 展開まで通るようにすること
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
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
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

/// 同一バージョンは `UpToDate` になるようにすること
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
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

/// digest 欠落時は確認がエラーになるようにすること
#[test]
fn update_flow_errors_when_digest_missing() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, size) = build_portable_zip_fixture(b"no-digest");

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(&tag, &asset_name, size, None));
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

/// SHA-256 不一致時は適用せずエラーになるようにすること
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
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
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

/// サイズ不一致時は適用せずエラーになるようにすること
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
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains("サイズ"));
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }

    cleanup_update_work_dir(&version);
}

/// 応答本文が宣言サイズを超える場合は途中で打ち切り適用しないようにすること
#[test]
fn update_flow_aborts_download_when_body_exceeds_declared_size() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, digest, size) = build_portable_zip_fixture(b"oversized-body");
    assert!(size > 1);

    let bridge = MockReleaseBridge::start();
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
    let mut oversized = zip_bytes;
    oversized.extend_from_slice(&[0u8; 4096]);

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

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
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

/// version API が 404 のとき確認がエラーになるようにすること
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

/// キー必須のアセットを正しい Bearer で適用できるようにすること
#[test]
fn update_flow_applies_with_required_download_key() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let marker = b"keyed-portable-ok";
    let (zip_bytes, digest, size) = build_portable_zip_fixture(marker);
    let key = "test-clip-refiner-key";

    let bridge = MockReleaseBridge::start();
    bridge.set_required_asset_key(key);
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
    bridge.set_asset(&tag, &asset_name, zip_bytes);

    // version API はキー不要のまま確認できる
    let update = match check_update_at(
        bridge.base_url(),
        "0.1.0",
        UpdatePackageKind::PortableZip,
        std::env::consts::ARCH,
    ) {
        UpdateCheckOutcome::Available(update) => update,
        other => panic!("想定外の確認結果: {other:?}"),
    };

    let apply =
        apply_update_for_test_with_key(&update, bridge.base_url(), Some(key), &[trusted_key()]);
    let path = match apply {
        UpdateApplyOutcome::PortableExtracted {
            path,
            helper_launched,
        } => {
            assert!(!helper_launched);
            PathBuf::from(path)
        }
        other => panic!("想定外の適用結果: {other:?}"),
    };
    let content = fs::read_to_string(path.join("PORTABLE.txt")).expect("PORTABLE.txt");
    assert_eq!(content.as_bytes(), marker);

    cleanup_update_work_dir(&version);
}

/// キー必須のアセットをキー無しで適用すると認証エラーになるようにすること
#[test]
fn update_flow_errors_when_download_key_missing() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, digest, size) = build_portable_zip_fixture(b"keyed-denied");

    let bridge = MockReleaseBridge::start();
    bridge.set_required_asset_key("expected-key");
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

    let apply = apply_update_for_test_with_key(&update, bridge.base_url(), None, &[trusted_key()]);
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains("認証"));
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }

    cleanup_update_work_dir(&version);
}

/// キー必須のアセットを誤ったキーで適用すると認証エラーになるようにすること
#[test]
fn update_flow_errors_when_download_key_wrong() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, digest, size) = build_portable_zip_fixture(b"keyed-wrong");

    let bridge = MockReleaseBridge::start();
    bridge.set_required_asset_key("expected-key");
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    publish_signature(&bridge, &tag, &version, &asset_name, &zip_bytes);
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

    let apply = apply_update_for_test_with_key(
        &update,
        bridge.base_url(),
        Some("wrong-key"),
        &[trusted_key()],
    );
    match apply {
        UpdateApplyOutcome::Error { message } => {
            assert!(message.contains("認証"));
        }
        other => panic!("想定外の適用結果: {other:?}"),
    }

    cleanup_update_work_dir(&version);
}

/// `msiexec` を起動せず MSI アセットのダウンロード検証まで通るようにすること
#[test]
fn update_flow_check_and_verify_msi_without_launch() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = msi_asset_name(&version);
    let payload = b"fake-msi-payload";
    let digest = sha256_hex(payload);
    let size = payload.len() as u64;

    let bridge = MockReleaseBridge::start();
    bridge.set_version_json(version_api_json(
        &tag,
        &asset_name,
        size,
        Some(&format!("sha256:{digest}")),
    ));
    publish_signature(&bridge, &tag, &version, &asset_name, payload);
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

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
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

/// 署名ファイルが無いときはアセットを取得せずエラーになるようにすること
#[test]
fn update_flow_rejects_missing_signature() {
    let version = unique_version();
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, _size) = build_portable_zip_fixture(b"no-signature");

    let bridge = MockReleaseBridge::start();
    let update = check_portable_update(&bridge, &version, &asset_name, zip_bytes);

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
    assert_apply_error(apply, "署名の取得");
    assert!(!downloaded_archive_path(&version, &asset_name).exists());

    cleanup_update_work_dir(&version);
}

/// 信頼しない鍵の署名ではアセットを適用せず削除すること
#[test]
fn update_flow_rejects_signature_from_untrusted_key() {
    let version = unique_version();
    let tag = format!("v{version}");
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, _size) = build_portable_zip_fixture(b"untrusted-signer");

    let bridge = MockReleaseBridge::start();
    let attacker = KeyPair::generate_unencrypted_keypair().expect("keypair");
    let comment = update_signature_trusted_comment(&version, &asset_name);
    bridge.set_asset(
        &tag,
        &update_signature_asset_name(&asset_name),
        sign_with(&attacker, &zip_bytes, &comment),
    );
    let update = check_portable_update(&bridge, &version, &asset_name, zip_bytes);

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
    assert_apply_error(apply, "信頼された鍵");
    assert!(!downloaded_archive_path(&version, &asset_name).exists());

    cleanup_update_work_dir(&version);
}

/// digest が一致しても署名と内容が合わなければ適用しないこと
///
/// version API とアセットの両方を差し替えられた場合を想定する
#[test]
fn update_flow_rejects_asset_replaced_with_matching_digest() {
    let version = unique_version();
    let asset_name = portable_asset_name(&version);
    let (original, _digest, _size) = build_portable_zip_fixture(b"original");
    let (replaced, _digest, _size) = build_portable_zip_fixture(b"replaced-by-attacker");

    let bridge = MockReleaseBridge::start();
    publish_signature(
        &bridge,
        &format!("v{version}"),
        &version,
        &asset_name,
        &original,
    );
    // digest は差し替え後のアセットに合わせるため SHA-256 検証は通る
    let update = check_portable_update(&bridge, &version, &asset_name, replaced);

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
    assert_apply_error(apply, "署名検証");
    assert!(!downloaded_archive_path(&version, &asset_name).exists());

    cleanup_update_work_dir(&version);
}

/// 古い版の正規署名付きアセットを新しい版として配られても適用しないこと
#[test]
fn update_flow_rejects_signature_for_other_version() {
    let old_version = unique_version();
    let version = unique_version();
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, _size) = build_portable_zip_fixture(b"rollback");

    let bridge = MockReleaseBridge::start();
    // 旧版として署名された内容を新しいタグ・アセット名で配る
    let comment =
        update_signature_trusted_comment(&old_version, &portable_asset_name(&old_version));
    bridge.set_asset(
        &format!("v{version}"),
        &update_signature_asset_name(&asset_name),
        sign_with(signing_key_pair(), &zip_bytes, &comment),
    );
    let update = check_portable_update(&bridge, &version, &asset_name, zip_bytes);

    let apply = apply_update_for_test(&update, bridge.base_url(), &[trusted_key()]);
    assert_apply_error(apply, "一致しない");
    assert!(!downloaded_archive_path(&version, &asset_name).exists());

    cleanup_update_work_dir(&version);
}

/// 信頼する公開鍵が無いビルドでは更新を適用しないこと
#[test]
fn update_flow_rejects_when_no_trusted_key() {
    let version = unique_version();
    let asset_name = portable_asset_name(&version);
    let (zip_bytes, _digest, _size) = build_portable_zip_fixture(b"no-trusted-key");

    let bridge = MockReleaseBridge::start();
    publish_signature(
        &bridge,
        &format!("v{version}"),
        &version,
        &asset_name,
        &zip_bytes,
    );
    let update = check_portable_update(&bridge, &version, &asset_name, zip_bytes);

    let apply = apply_update_for_test(&update, bridge.base_url(), &[]);
    assert_apply_error(apply, "公開鍵");
    assert!(!downloaded_archive_path(&version, &asset_name).exists());

    cleanup_update_work_dir(&version);
}
