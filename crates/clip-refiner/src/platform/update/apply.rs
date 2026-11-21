use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use super::error::UpdateError;
use super::replace::{current_install_dir, launch_portable_replace_helper};
use super::signature::{
    download_signature, embedded_signing_keys, signature_asset_name, verify_file_signature,
};
use super::types::{AvailableUpdate, UpdateApplyOutcome};
use super::verify::{download_and_verify_with_key, validate_asset_name};
use super::{
    BRIDGE_BASE_URL, RELEASE_OWNER, RELEASE_REPO, UPDATE_WORK_DIR_NAME, bridge_download_key,
};

use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use zip::ZipArchive;

// ======================================================================
// パブリック関数
// ======================================================================
/// 更新適用時の副作用オプション
#[derive(Debug, Clone, Copy)]
pub struct ApplyOptions {
    /// ポータブル差し替えヘルパー起動失敗時にエクスプローラーを開くか
    pub open_explorer: bool,
    /// MSI 取得後に `msiexec` を起動するか
    pub launch_msi: bool,
    /// ポータブル展開後に差し替えヘルパーを起動するか
    pub launch_replace_helper: bool,
}

impl ApplyOptions {
    /// 本番向け (`msiexec` / 差し替えヘルパーを起動する)
    pub const PRODUCTION: Self = Self {
        open_explorer: true,
        launch_msi: true,
        launch_replace_helper: true,
    };

    /// 統合テスト向け (外部プロセスを起動しない)
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub const TEST: Self = Self {
        open_explorer: false,
        launch_msi: false,
        launch_replace_helper: false,
    };
}

/// 更新アセットをダウンロードして適用する
///
/// 署名・サイズ・SHA-256 を検証してから、MSI は `msiexec` を起動する。
/// ポータブル ZIP は展開し、差し替えヘルパーを起動する
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
///
/// # Returns
/// * `UpdateApplyOutcome` - 適用結果
pub(crate) fn apply_update(update: &AvailableUpdate) -> UpdateApplyOutcome {
    apply_update_at(
        update,
        BRIDGE_BASE_URL,
        ApplyOptions::PRODUCTION,
        embedded_signing_keys(),
    )
}

/// 指定した bridge ベース URL で更新アセットをダウンロードして適用する
///
/// 統合テストからローカル HTTP サーバへ向けて呼び出す
/// ダウンロードキーはビルド埋め込み値を使う
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
/// * `base_url` - bridge のベース URL (末尾スラッシュなし)
/// * `options` - 外部プロセス起動の有無
/// * `trusted_keys` - 署名検証に使う minisign 公開鍵 (Base64)
///
/// # Returns
/// * `UpdateApplyOutcome` - 適用結果
pub(crate) fn apply_update_at(
    update: &AvailableUpdate,
    base_url: &str,
    options: ApplyOptions,
    trusted_keys: &[&str],
) -> UpdateApplyOutcome {
    let download_key = bridge_download_key();
    apply_update_with(
        update,
        base_url,
        options,
        download_key.as_ref().map(|key| key.as_str()),
        trusted_keys,
    )
}

/// 指定キーで bridge から更新アセットをダウンロードして適用する
///
/// 統合テストからキー有無を差し替えるときに使う
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
/// * `base_url` - bridge のベース URL (末尾スラッシュなし)
/// * `options` - 外部プロセス起動の有無
/// * `download_key` - bridge ダウンロードキー (`None` なら無認証)
/// * `trusted_keys` - 署名検証に使う minisign 公開鍵 (Base64)
///
/// # Returns
/// * `UpdateApplyOutcome` - 適用結果
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn apply_update_at_with_key(
    update: &AvailableUpdate,
    base_url: &str,
    options: ApplyOptions,
    download_key: Option<&str>,
    trusted_keys: &[&str],
) -> UpdateApplyOutcome {
    apply_update_with(update, base_url, options, download_key, trusted_keys)
}

/// ダウンロード・検証・適用を順に行う
fn apply_update_with(
    update: &AvailableUpdate,
    base_url: &str,
    options: ApplyOptions,
    download_key: Option<&str>,
    trusted_keys: &[&str],
) -> UpdateApplyOutcome {
    // 確認時にも検証済みだが、保存先パスと URL に使う直前でも再確認する
    if let Err(err) = validate_asset_name(&update.asset_name) {
        return err.into();
    }
    let work_dir = match update_work_dir(&update.latest_version) {
        Ok(dir) => dir,
        Err(err) => return err.into(),
    };

    let archive_path = work_dir.join(&update.asset_name);
    if let Err(err) =
        download_verified_asset(update, base_url, &archive_path, download_key, trusted_keys)
    {
        return err.into();
    }

    apply_downloaded_asset(update, &archive_path, &work_dir, options)
}

/// アセットと署名をダウンロードし、サイズ・SHA-256・署名を検証する
///
/// 署名は小さいため先に取得し、欠落時はアセット本体をダウンロードしない
/// 署名検証に失敗した場合は保存済みアセットを削除する
///
/// # Arguments
/// * `update` - 確認済みの更新アセット情報
/// * `base_url` - bridge のベース URL (末尾スラッシュなし)
/// * `archive_path` - アセットの保存先
/// * `download_key` - bridge ダウンロードキー (`None` なら無認証)
/// * `trusted_keys` - 署名検証に使う minisign 公開鍵 (Base64)
///
/// # Returns
/// * `Result<(), UpdateError>` - すべての検証に成功した場合は `Ok(())`
fn download_verified_asset(
    update: &AvailableUpdate,
    base_url: &str,
    archive_path: &Path,
    download_key: Option<&str>,
    trusted_keys: &[&str],
) -> Result<(), UpdateError> {
    if trusted_keys.is_empty() {
        crate::log_warn!("更新署名の公開鍵がビルドに埋め込まれていない");
        return Err(UpdateError::SignatureKeyMissing);
    }

    let signature_url = asset_download_url(
        base_url,
        &update.tag,
        &signature_asset_name(&update.asset_name),
    );
    let signature = download_signature(&signature_url, download_key)?;

    let download_url = asset_download_url(base_url, &update.tag, &update.asset_name);
    download_and_verify_with_key(
        &download_url,
        archive_path,
        update.size,
        &update.sha256,
        download_key,
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

/// ダウンロード済みアセットを MSI / ポータブルへ振り分ける
fn apply_downloaded_asset(
    update: &AvailableUpdate,
    archive_path: &Path,
    work_dir: &Path,
    options: ApplyOptions,
) -> UpdateApplyOutcome {
    if Path::new(&update.asset_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
    {
        if options.launch_msi {
            return launch_msi(archive_path);
        }
        return UpdateApplyOutcome::MsiStarted;
    }

    if update
        .asset_name
        .to_ascii_lowercase()
        .ends_with("-portable.zip")
    {
        return extract_portable_zip(archive_path, work_dir, options);
    }

    UpdateError::UnsupportedAsset {
        name: update.asset_name.clone(),
    }
    .into()
}

/// アセットダウンロード URL を組み立てる
///
/// # Arguments
/// * `base_url` - bridge のベース URL (末尾スラッシュなし)
/// * `tag` - リリースタグ
/// * `asset_name` - アセット名
///
/// # Returns
/// * `String` - ダウンロード URL
pub(crate) fn asset_download_url(base_url: &str, tag: &str, asset_name: &str) -> String {
    format!("{base_url}/{RELEASE_OWNER}/{RELEASE_REPO}/{tag}/{asset_name}")
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 更新作業用ディレクトリを用意する
fn update_work_dir(version: &str) -> Result<PathBuf, UpdateError> {
    let base = std::env::temp_dir()
        .join(UPDATE_WORK_DIR_NAME)
        .join(version);
    fs::create_dir_all(&base).map_err(|err| {
        crate::log_warn!("更新作業ディレクトリの作成に失敗: {err}");
        UpdateError::WorkDirCreate
    })?;
    Ok(base)
}

/// MSI を `msiexec` で起動する
///
/// 実行ファイルと同じフォルダの同名ファイルを拾わないよう、システムフォルダの絶対パスで起動する
fn launch_msi(msi_path: &Path) -> UpdateApplyOutcome {
    let Some(path_str) = msi_path.to_str() else {
        return UpdateError::MsiPathInvalid.into();
    };
    let Some(msiexec) = system_msiexec_path() else {
        crate::log_warn!("システムフォルダの取得に失敗");
        return UpdateError::MsiLaunch.into();
    };

    match std::process::Command::new(msiexec)
        .args(["/i", path_str])
        .spawn()
    {
        Ok(_) => UpdateApplyOutcome::MsiStarted,
        Err(err) => {
            crate::log_warn!("msiexec の起動に失敗: {err}");
            UpdateError::MsiLaunch.into()
        }
    }
}

/// システムフォルダの `msiexec.exe` の絶対パスを返す
///
/// # Returns
/// * `Option<PathBuf>` - 取得できない場合は `None`
fn system_msiexec_path() -> Option<PathBuf> {
    let mut buf = [0u16; 260];
    // SAFETY: `buf` は呼び出し中有効な書き込み可能バッファで、長さはスライスから渡す
    let len = usize::try_from(unsafe { GetSystemDirectoryW(Some(&mut buf)) }).ok()?;
    if len == 0 || len >= buf.len() {
        return None;
    }
    let dir = PathBuf::from(String::from_utf16(&buf[..len]).ok()?);
    Some(dir.join("msiexec.exe"))
}

/// ポータブル ZIP を展開し、可能なら差し替えヘルパーを起動する
fn extract_portable_zip(
    zip_path: &Path,
    work_dir: &Path,
    options: ApplyOptions,
) -> UpdateApplyOutcome {
    let extract_dir = work_dir.join("extracted");
    if extract_dir.exists()
        && let Err(err) = fs::remove_dir_all(&extract_dir)
    {
        crate::log_warn!("旧展開ディレクトリの削除に失敗: {err}");
        return UpdateError::ExtractDirPrepare.into();
    }
    if let Err(err) = fs::create_dir_all(&extract_dir) {
        crate::log_warn!("展開ディレクトリの作成に失敗: {err}");
        return UpdateError::ExtractDirCreate.into();
    }

    if let Err(err) = extract_zip_archive(zip_path, &extract_dir) {
        return err.into();
    }

    let open_path = find_portable_root(&extract_dir).unwrap_or_else(|| extract_dir.clone());
    let mut helper_launched = false;
    if options.launch_replace_helper {
        match current_install_dir() {
            Ok(install_dir) => {
                match launch_portable_replace_helper(&open_path, &install_dir, work_dir) {
                    Ok(()) => helper_launched = true,
                    Err(err) => {
                        crate::log_warn!("ポータブル差し替えヘルパー起動失敗: {err}");
                    }
                }
            }
            Err(err) => {
                crate::log_warn!("実行フォルダの特定に失敗: {err}");
            }
        }
    }

    if !helper_launched && options.open_explorer {
        open_in_explorer(&open_path);
    }

    UpdateApplyOutcome::PortableExtracted {
        path: open_path.display().to_string(),
        helper_launched,
    }
}

/// ZIP を安全に展開する (Zip Slip 対策付き)
fn extract_zip_archive(zip_path: &Path, dest_dir: &Path) -> Result<(), UpdateError> {
    let file = File::open(zip_path).map_err(|err| {
        crate::log_warn!("ZIP オープン失敗: {err}");
        UpdateError::ZipOpen
    })?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        crate::log_warn!("ZIP 解釈失敗: {err}");
        UpdateError::ZipInvalid
    })?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| {
            crate::log_warn!("ZIP エントリ取得失敗: {err}");
            UpdateError::ZipExtract
        })?;
        let Some(enclosed) = entry.enclosed_name() else {
            crate::log_warn!("不正な ZIP エントリ名をスキップ: {}", entry.name());
            continue;
        };
        let out_path = dest_dir.join(enclosed);

        if entry.name().ends_with('/') {
            fs::create_dir_all(&out_path).map_err(|err| {
                crate::log_warn!("ZIP ディレクトリ作成失敗: {err}");
                UpdateError::ZipExtract
            })?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                crate::log_warn!("ZIP 親ディレクトリ作成失敗: {err}");
                UpdateError::ZipExtract
            })?;
        }

        let mut out_file = File::create(&out_path).map_err(|err| {
            crate::log_warn!("ZIP 出力ファイル作成失敗: {err}");
            UpdateError::ZipExtract
        })?;
        io::copy(&mut entry, &mut out_file).map_err(|err| {
            crate::log_warn!("ZIP 展開書き込み失敗: {err}");
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

/// エクスプローラーでフォルダを開く
fn open_in_explorer(path: &Path) {
    if let Err(err) = std::process::Command::new("explorer").arg(path).spawn() {
        crate::log_warn!("エクスプローラー起動失敗: {err}");
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    /// ダウンロード URL が bridge パス規約に従うようにすること
    #[test]
    fn asset_download_url_matches_bridge_convention() {
        let url = asset_download_url(BRIDGE_BASE_URL, "v0.13.1", "clip-refiner-0.13.1-x86_64.msi");
        assert_eq!(
            url,
            "https://gh-release-bridge.gnudlemna-4656.workers.dev/GnudMna/clip-refiner/v0.13.1/clip-refiner-0.13.1-x86_64.msi"
        );
    }

    /// `msiexec` をシステムフォルダの絶対パスで解決できるようにすること
    #[test]
    fn system_msiexec_path_is_absolute_and_exists() {
        let path = system_msiexec_path().expect("msiexec path");
        assert!(path.is_absolute());
        assert!(path.is_file(), "{}", path.display());
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
    fn apply_update_rejects_path_like_asset_name() {
        let update = sample_update("../clip-refiner-0.14.3-x86_64.msi");

        // 接続できない URL でも検証で先に失敗するためネットワークへは出ない
        let outcome = apply_update_with(
            &update,
            "http://127.0.0.1:9",
            ApplyOptions::TEST,
            None,
            &["unused"],
        );
        match outcome {
            UpdateApplyOutcome::Error { message } => {
                assert_eq!(message, UpdateError::AssetNameInvalid.to_string());
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    /// MSI で `launch_msi` 無効時は起動せず `MsiStarted` を返すようにすること
    #[test]
    fn apply_downloaded_msi_skips_launch_when_disabled() {
        let update = sample_update("clip-refiner-0.14.3-x86_64.msi");
        let dummy = PathBuf::from("unused.msi");
        let work = PathBuf::from("unused-work");

        let outcome = apply_downloaded_asset(&update, &dummy, &work, ApplyOptions::TEST);
        assert!(matches!(outcome, UpdateApplyOutcome::MsiStarted));
    }

    /// 非 MSI・非ポータブル ZIP は未対応アセットとして失敗すること
    #[test]
    fn apply_downloaded_asset_rejects_unsupported_name() {
        let update = sample_update("clip-refiner-0.14.3.zip");
        let dummy = PathBuf::from("unused.zip");
        let work = PathBuf::from("unused-work");

        let outcome = apply_downloaded_asset(&update, &dummy, &work, ApplyOptions::TEST);
        match outcome {
            UpdateApplyOutcome::Error { message } => {
                assert!(message.contains("clip-refiner-0.14.3.zip") || message.contains("未対応"));
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    /// ポータブル ZIP を展開しヘルパー起動なしで完了すること
    #[test]
    fn apply_downloaded_portable_extracts_without_helper() {
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
        let outcome = apply_downloaded_asset(&update, &zip_path, &root, ApplyOptions::TEST);
        match outcome {
            UpdateApplyOutcome::PortableExtracted {
                path,
                helper_launched,
            } => {
                assert!(!helper_launched);
                assert!(
                    Path::new(&path).join("PORTABLE.txt").is_file() || Path::new(&path).is_dir()
                );
            }
            other => panic!("expected PortableExtracted, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&root);
    }
}
