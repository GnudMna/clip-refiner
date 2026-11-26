//! 更新アセットのダウンロード・検証・展開
//!
//! インストール先には触れない。ここで失敗・中断しても、動作中のアプリに影響しない

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use super::error::UpdateError;
use super::signature::{
    download_signature, embedded_signing_keys, signature_asset_name, verify_file_signature,
};
use super::types::{AvailableUpdate, PreparedPackage};
use super::verify::{download_and_verify, validate_asset_name};
use super::{GITHUB_BASE_URL, RELEASE_OWNER, RELEASE_REPO, UPDATE_WORK_DIR_NAME};

// ======================================================================
// パブリック関数
// ======================================================================
/// 更新アセットをダウンロードし、検証と展開まで済ませる
///
/// 署名・サイズ・SHA-256 のすべての検証に通ったものだけを返す
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
/// * `on_progress` - 受信済みバイト数の通知先
///
/// # Returns
/// * `Result<PreparedPackage, UpdateError>` - インストールできる状態の配布物
pub(crate) fn prepare_update(
    update: &AvailableUpdate,
    on_progress: &mut dyn FnMut(u64),
) -> Result<PreparedPackage, UpdateError> {
    prepare_update_at(
        update,
        GITHUB_BASE_URL,
        embedded_signing_keys(),
        on_progress,
    )
}

/// 指定したベース URL から更新アセットをダウンロードし、検証と展開まで済ませる
///
/// 統合テストからローカル HTTP サーバへ向けて呼び出す
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
/// * `base_url` - GitHub のベース URL (末尾スラッシュなし)
/// * `trusted_keys` - 署名検証に使う minisign 公開鍵 (Base64)
/// * `on_progress` - 受信済みバイト数の通知先
///
/// # Returns
/// * `Result<PreparedPackage, UpdateError>` - インストールできる状態の配布物
pub(crate) fn prepare_update_at(
    update: &AvailableUpdate,
    base_url: &str,
    trusted_keys: &[&str],
    on_progress: &mut dyn FnMut(u64),
) -> Result<PreparedPackage, UpdateError> {
    // 確認時にも検証済みだが、保存先パスと URL に使う直前でも再確認する
    validate_asset_name(&update.asset_name)?;
    let work_dir = update_work_dir(&update.latest_version)?;

    let archive_path = work_dir.join(&update.asset_name);
    download_verified_asset(update, base_url, &archive_path, trusted_keys, on_progress)?;

    open_verified_asset(update, &archive_path, &work_dir)
}

/// ダウンロードと展開に使った作業フォルダを削除する
///
/// 削除できなければ残し、次回の常駐の起動時に掃除する
///
/// # Arguments
/// * `version` - 更新先のバージョン
pub(crate) fn remove_work_dir(version: &str) {
    let dir = work_dir_path(version);
    if let Err(err) = fs::remove_dir_all(&dir)
        && err.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!("更新作業ディレクトリの削除に失敗: {err}");
    }
}

/// アセットと署名をダウンロードし、サイズ・SHA-256・署名を検証する
///
/// 署名は小さいため先に取得し、欠落時はアセット本体をダウンロードしない
/// 署名検証に失敗した場合は保存済みアセットを削除する
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
/// * `base_url` - GitHub のベース URL (末尾スラッシュなし)
/// * `archive_path` - アセットの保存先
/// * `trusted_keys` - 署名検証に使う minisign 公開鍵 (Base64)
/// * `on_progress` - 受信済みバイト数の通知先
///
/// # Returns
/// * `Result<(), UpdateError>` - すべての検証に成功した場合は `Ok(())`
fn download_verified_asset(
    update: &AvailableUpdate,
    base_url: &str,
    archive_path: &Path,
    trusted_keys: &[&str],
    on_progress: &mut dyn FnMut(u64),
) -> Result<(), UpdateError> {
    if trusted_keys.is_empty() {
        tracing::warn!("更新署名の公開鍵がビルドに埋め込まれていない");
        return Err(UpdateError::SignatureKeyMissing);
    }

    let signature_url = asset_download_url(
        base_url,
        &update.tag,
        &signature_asset_name(&update.asset_name),
    );
    let signature = download_signature(&signature_url)?;

    let download_url = asset_download_url(base_url, &update.tag, &update.asset_name);
    download_and_verify(
        &download_url,
        archive_path,
        update.size,
        &update.sha256,
        on_progress,
    )?;

    let verified = verify_file_signature(
        archive_path,
        &signature,
        &update.latest_version,
        &update.asset_name,
        trusted_keys,
    );
    if verified.is_err() {
        let _ = fs::remove_file(archive_path);
    }
    verified
}

/// 検証済みアセットを種類に応じてインストールできる状態にする
///
/// MSI はそのまま使い、ポータブル ZIP は展開する
fn open_verified_asset(
    update: &AvailableUpdate,
    archive_path: &Path,
    work_dir: &Path,
) -> Result<PreparedPackage, UpdateError> {
    if Path::new(&update.asset_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
    {
        return Ok(PreparedPackage::Msi {
            path: archive_path.to_path_buf(),
        });
    }

    if update
        .asset_name
        .to_ascii_lowercase()
        .ends_with("-portable.zip")
    {
        let source_dir = extract_portable_zip(archive_path, work_dir)?;
        return Ok(PreparedPackage::Portable { source_dir });
    }

    Err(UpdateError::UnsupportedAsset {
        name: update.asset_name.clone(),
    })
}

/// アセットダウンロード URL を組み立てる
///
/// # Arguments
/// * `base_url` - GitHub のベース URL (末尾スラッシュなし)
/// * `tag` - リリースタグ
/// * `asset_name` - アセット名
///
/// # Returns
/// * `String` - ダウンロード URL
pub(crate) fn asset_download_url(base_url: &str, tag: &str, asset_name: &str) -> String {
    format!("{base_url}/{RELEASE_OWNER}/{RELEASE_REPO}/releases/download/{tag}/{asset_name}")
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 更新先のバージョンに対応する作業ディレクトリのパスを返す
fn work_dir_path(version: &str) -> PathBuf {
    std::env::temp_dir()
        .join(UPDATE_WORK_DIR_NAME)
        .join(version)
}

/// 更新作業用ディレクトリを用意する
fn update_work_dir(version: &str) -> Result<PathBuf, UpdateError> {
    let base = work_dir_path(version);
    fs::create_dir_all(&base).map_err(|err| {
        tracing::warn!("更新作業ディレクトリの作成に失敗: {err}");
        UpdateError::WorkDirCreate
    })?;
    Ok(base)
}

/// ポータブル ZIP を展開し、配布物のルートを返す
fn extract_portable_zip(zip_path: &Path, work_dir: &Path) -> Result<PathBuf, UpdateError> {
    let extract_dir = work_dir.join("extracted");
    if extract_dir.exists()
        && let Err(err) = fs::remove_dir_all(&extract_dir)
    {
        tracing::warn!("旧展開ディレクトリの削除に失敗: {err}");
        return Err(UpdateError::ExtractDirPrepare);
    }
    fs::create_dir_all(&extract_dir).map_err(|err| {
        tracing::warn!("展開ディレクトリの作成に失敗: {err}");
        UpdateError::ExtractDirCreate
    })?;

    extract_zip_archive(zip_path, &extract_dir)?;

    Ok(find_portable_root(&extract_dir).unwrap_or(extract_dir))
}

/// ZIP を安全に展開する (Zip Slip 対策付き)
fn extract_zip_archive(zip_path: &Path, dest_dir: &Path) -> Result<(), UpdateError> {
    let file = File::open(zip_path).map_err(|err| {
        tracing::warn!("ZIP オープン失敗: {err}");
        UpdateError::ZipOpen
    })?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        tracing::warn!("ZIP 解釈失敗: {err}");
        UpdateError::ZipInvalid
    })?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| {
            tracing::warn!("ZIP エントリ取得失敗: {err}");
            UpdateError::ZipExtract
        })?;
        let Some(enclosed) = entry.enclosed_name() else {
            tracing::warn!(
                "不正な ZIP エントリ名をスキップ: {}",
                String::from_utf8_lossy(entry.name_raw())
            );
            continue;
        };
        let out_path = dest_dir.join(enclosed);

        if entry.name_raw().ends_with(b"/") {
            fs::create_dir_all(&out_path).map_err(|err| {
                tracing::warn!("ZIP ディレクトリ作成失敗: {err}");
                UpdateError::ZipExtract
            })?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                tracing::warn!("ZIP 親ディレクトリ作成失敗: {err}");
                UpdateError::ZipExtract
            })?;
        }

        let mut out_file = File::create(&out_path).map_err(|err| {
            tracing::warn!("ZIP 出力ファイル作成失敗: {err}");
            UpdateError::ZipExtract
        })?;
        io::copy(&mut entry, &mut out_file).map_err(|err| {
            tracing::warn!("ZIP 展開書き込み失敗: {err}");
            UpdateError::ZipExtract
        })?;
    }

    Ok(())
}

/// 展開先直下の単一フォルダがあればそれを返す
fn find_portable_root(extract_dir: &Path) -> Option<PathBuf> {
    let mut dirs = Vec::new();
    let mut files = 0usize;
    let entries = fs::read_dir(extract_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else {
            files += 1;
        }
    }
    if files == 0 && dirs.len() == 1 {
        Some(dirs.remove(0))
    } else {
        None
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::*;

    /// ダウンロード URL が GitHub Releases のパスに従うようにすること
    #[test]
    fn asset_download_url_matches_github_releases() {
        let url = asset_download_url(GITHUB_BASE_URL, "v0.13.1", "clip-refiner-0.13.1-x86_64.msi");
        assert_eq!(
            url,
            "https://github.com/GnudMna/clip-refiner/releases/download/v0.13.1/clip-refiner-0.13.1-x86_64.msi"
        );
    }

    /// ZIP を展開してファイル内容を復元できるようにすること
    #[test]
    fn extract_zip_archive_writes_nested_files() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-extract-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let zip_path = root.join("sample.zip");
        let dest = root.join("out");

        {
            let file = File::create(&zip_path).expect("zip create");
            let mut writer = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer
                .start_file("pkg/PORTABLE.txt", options)
                .expect("start file");
            writer.write_all(b"portable-ok").expect("write");
            writer.finish().expect("finish zip");
        }

        extract_zip_archive(&zip_path, &dest).expect("extract");
        let content = fs::read_to_string(dest.join("pkg").join("PORTABLE.txt")).expect("read");
        assert_eq!(content, "portable-ok");

        let _ = fs::remove_dir_all(&root);
    }

    /// Zip Slip なエントリを展開先外へ書かないようにすること
    #[test]
    fn extract_zip_archive_skips_zip_slip_entries() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-slip-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let zip_path = root.join("slip.zip");
        let dest = root.join("out");
        fs::create_dir_all(&dest).expect("dest");

        {
            let file = File::create(&zip_path).expect("zip create");
            let mut writer = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer
                .start_file("../evil.txt", options)
                .expect("start slip file");
            writer.write_all(b"evil").expect("write");
            writer
                .start_file("safe.txt", options)
                .expect("start safe file");
            writer.write_all(b"safe").expect("write");
            writer.finish().expect("finish zip");
        }

        extract_zip_archive(&zip_path, &dest).expect("extract");
        assert!(dest.join("safe.txt").is_file());
        assert!(!root.join("evil.txt").exists());

        let _ = fs::remove_dir_all(&root);
    }

    /// 展開直下の単一フォルダをポータブルルートとして返すようにすること
    #[test]
    fn find_portable_root_returns_single_child_dir() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-root-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let nested = root.join("clip-refiner-0.13.1-portable");
        fs::create_dir_all(&nested).expect("nested");
        fs::write(nested.join("ClipRefiner.exe"), b"x").expect("exe");

        let found = find_portable_root(&root).expect("portable root");
        assert_eq!(found, nested);

        let _ = fs::remove_dir_all(&root);
    }

    /// 直下にファイルがある場合はルートを返さないようにすること
    #[test]
    fn find_portable_root_none_when_files_present() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-root-files-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("ClipRefiner.exe"), b"x").expect("exe");

        assert!(find_portable_root(&root).is_none());

        let _ = fs::remove_dir_all(&root);
    }

    fn sample_update(asset_name: &str) -> AvailableUpdate {
        AvailableUpdate {
            current_version: "0.14.0".to_string(),
            latest_version: "0.14.3".to_string(),
            tag: "v0.14.3".to_string(),
            asset_name: asset_name.to_string(),
            size: 1,
            sha256: "a".repeat(64),
        }
    }

    /// 不正なアセット名ではダウンロードせずに失敗すること
    #[test]
    fn prepare_update_rejects_path_like_asset_name() {
        let update = sample_update("../clip-refiner-0.14.3-x86_64.msi");

        // 接続できない URL でも検証で先に失敗するためネットワークへは出ない
        let result = prepare_update_at(&update, "http://127.0.0.1:9", &["unused"], &mut |_| {});
        assert_eq!(result, Err(UpdateError::AssetNameInvalid));
    }

    /// MSI は展開せず、ダウンロードしたファイルをそのまま返すこと
    #[test]
    fn open_verified_msi_returns_archive_path() {
        let update = sample_update("clip-refiner-0.14.3-x86_64.msi");
        let path = PathBuf::from("unused.msi");

        let package = open_verified_asset(&update, &path, Path::new("unused-work"));
        assert_eq!(package, Ok(PreparedPackage::Msi { path }));
    }

    /// 非 MSI・非ポータブル ZIP は未対応アセットとして失敗すること
    #[test]
    fn open_verified_asset_rejects_unsupported_name() {
        let update = sample_update("clip-refiner-0.14.3.zip");

        let result = open_verified_asset(&update, Path::new("unused.zip"), Path::new("unused"));
        assert_eq!(
            result,
            Err(UpdateError::UnsupportedAsset {
                name: "clip-refiner-0.14.3.zip".to_string()
            })
        );
    }

    /// ポータブル ZIP を展開し、配布物のルートを返すこと
    #[test]
    fn open_verified_portable_extracts_payload() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-apply-portable-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let zip_path = root.join("clip-refiner-0.14.3-portable.zip");

        {
            let file = File::create(&zip_path).expect("zip create");
            let mut writer = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer
                .start_file("clip-refiner/PORTABLE.txt", options)
                .expect("start file");
            writer.write_all(b"portable").expect("write");
            writer.finish().expect("finish zip");
        }

        let update = sample_update("clip-refiner-0.14.3-portable.zip");
        match open_verified_asset(&update, &zip_path, &root) {
            Ok(PreparedPackage::Portable { source_dir }) => {
                assert!(source_dir.join("PORTABLE.txt").is_file());
            }
            other => panic!("expected Portable, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&root);
    }
}
