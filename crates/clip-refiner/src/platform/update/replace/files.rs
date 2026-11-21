//! 差し替えヘルパーのファイル操作
//!
//! 実行ファイルの探索と、展開済み配布物のインストール先への反映を担う
//! 反映は旧ファイルを退避しながら進め、失敗したら差し替え前の状態へ戻す
//! 利用者データ (`data`) は反映対象から外す

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::error::UpdateError;
use crate::consts::{APP_NAME, PORTABLE_DATA_DIR_NAME};

// ======================================================================
// 定数
// ======================================================================
/// 差し替え中に旧ファイルを退避する、差し替え先直下のフォルダ名
const BACKUP_DIR_NAME: &str = ".clip-refiner-update-backup";

// ======================================================================
// 実行ファイル
// ======================================================================
/// 実行ファイル名 (`ClipRefiner.exe`)
pub(super) fn portable_exe_name() -> String {
    format!("{APP_NAME}.exe")
}

/// 自身の実行ディレクトリを返す
pub(super) fn current_exe_dir() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe().map_err(|err| {
        crate::log_warn!("実行ファイルパスの取得に失敗: {err}");
        UpdateError::ExePath
    })?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or(UpdateError::ExeDir)
}

/// 展開ルート直下の `ClipRefiner.exe` を探す
pub(super) fn find_portable_exe(source_dir: &Path) -> Option<PathBuf> {
    let direct = source_dir.join(portable_exe_name());
    if direct.is_file() {
        return Some(direct);
    }

    let entries = fs::read_dir(source_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(&portable_exe_name()) {
            return Some(path);
        }
    }
    None
}

/// パスをコマンドライン引数用の文字列にする
pub(super) fn path_to_string(path: &Path) -> Result<String, UpdateError> {
    path.to_str()
        .map(str::to_string)
        .ok_or(UpdateError::PathInvalid)
}

// ======================================================================
// 反映
// ======================================================================
/// ポータブル更新の配布物をインストール先へ反映する
///
/// 送信元の `data` はスキップする。宛先に既にある `data` は保持する
/// 差し替え先ディレクトリは既存である必要がある (新規作成しない)
/// 上書き・削除する宛先のパスは差し替え先内の退避フォルダへ移し、途中で失敗したら元へ戻す
/// 配布物のサブフォルダ (`docs` など) からは、新しい版に無いファイルを取り除く
/// 直下のファイルは上書きと追加のみ行い、利用者が置いたファイルを消さない
///
/// # Arguments
/// * `source_dir` - 展開ルート
/// * `dest_dir` - 差し替え先
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`。失敗時は差し替え前の状態へ戻したうえで
///   元のエラーを返し、戻せなかった場合は `UpdateError::RollbackFailed` を返す
pub(super) fn copy_portable_payload(source_dir: &Path, dest_dir: &Path) -> Result<(), UpdateError> {
    if !source_dir.is_dir() {
        return Err(UpdateError::SourceMissing);
    }
    if !dest_dir.is_dir() {
        crate::log_warn!("差し替え先がディレクトリではない: {}", dest_dir.display());
        return Err(UpdateError::DestPrepare);
    }

    let mut transaction = ReplaceTransaction::begin(dest_dir.join(BACKUP_DIR_NAME))?;
    match install_payload(source_dir, dest_dir, &mut transaction) {
        Ok(()) => {
            transaction.commit();
            Ok(())
        }
        Err(err) => {
            transaction.rollback()?;
            Err(err)
        }
    }
}

/// 展開ルート直下のエントリを順に反映する
fn install_payload(
    source_dir: &Path,
    dest_dir: &Path,
    transaction: &mut ReplaceTransaction,
) -> Result<(), UpdateError> {
    let entries = fs::read_dir(source_dir).map_err(|err| {
        crate::log_warn!("展開元の列挙に失敗: {err}");
        UpdateError::SourceRead
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| {
            crate::log_warn!("展開元エントリの取得に失敗: {err}");
            UpdateError::SourceRead
        })?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case(PORTABLE_DATA_DIR_NAME))
        {
            continue;
        }

        let from = entry.path();
        let to = dest_dir.join(&name);
        if from.is_dir() {
            transaction.sync_dir(&from, &to)?;
        } else {
            transaction.install_file(&from, &to)?;
        }
    }
    Ok(())
}

/// 差し替え中の変更履歴
enum Step {
    /// 新規に作ったファイル・ディレクトリ (巻き戻しで削除する)
    Created(PathBuf),
    /// 退避フォルダへ移した既存のパス (巻き戻しで元の場所へ戻す)
    Moved {
        /// 退避前のパス
        original: PathBuf,
        /// 退避先のパス
        backup: PathBuf,
    },
}

/// 差し替え先への変更を記録し、失敗時に差し替え前の状態へ戻す
struct ReplaceTransaction {
    backup_dir: PathBuf,
    steps: Vec<Step>,
    next_backup_id: usize,
}

impl ReplaceTransaction {
    /// 退避フォルダを作って開始する
    ///
    /// 前回の中断で残った退避フォルダは削除する
    fn begin(backup_dir: PathBuf) -> Result<Self, UpdateError> {
        remove_path(&backup_dir).map_err(|err| {
            crate::log_warn!("残っていた退避フォルダの削除に失敗: {err}");
            UpdateError::DestPrepare
        })?;
        fs::create_dir(&backup_dir).map_err(|err| {
            crate::log_warn!("退避フォルダの作成に失敗: {}: {err}", backup_dir.display());
            UpdateError::DestPrepare
        })?;
        Ok(Self {
            backup_dir,
            steps: Vec::new(),
            next_backup_id: 0,
        })
    }

    /// 成功を確定し、退避フォルダを削除する
    fn commit(self) {
        if let Err(err) = remove_path(&self.backup_dir) {
            crate::log_warn!("退避フォルダの削除に失敗: {err}");
        }
    }

    /// 記録を逆順にたどり、差し替え前の状態へ戻す
    ///
    /// 戻せない項目があった場合は退避フォルダを残し、その場所を `RollbackFailed` で伝える
    fn rollback(mut self) -> Result<(), UpdateError> {
        let mut failed = false;
        for step in std::mem::take(&mut self.steps).into_iter().rev() {
            let result = match &step {
                Step::Created(path) => remove_path(path),
                Step::Moved { original, backup } => fs::rename(backup, original),
            };
            if let Err(err) = result {
                crate::log_warn!("差し替えの巻き戻しに失敗: {err}");
                failed = true;
            }
        }
        if failed {
            return Err(UpdateError::RollbackFailed {
                backup: self.backup_dir.display().to_string(),
            });
        }
        self.commit();
        Ok(())
    }

    /// ディレクトリを再帰的に同期する
    ///
    /// 配布物のファイルを追加・上書きし、配布物に無い宛先のエントリは退避する
    fn sync_dir(&mut self, from: &Path, to: &Path) -> Result<(), UpdateError> {
        self.ensure_dir(to)?;

        let entries = fs::read_dir(from).map_err(|err| {
            crate::log_warn!("ディレクトリ列挙失敗: {}: {err}", from.display());
            UpdateError::DirReplace
        })?;

        let mut shipped = HashSet::new();
        for entry in entries {
            let entry = entry.map_err(|err| {
                crate::log_warn!("ディレクトリエントリ取得失敗: {err}");
                UpdateError::DirReplace
            })?;
            let name = entry.file_name();
            shipped.insert(name.to_string_lossy().to_lowercase());

            let from_path = entry.path();
            let to_path = to.join(&name);
            if from_path.is_dir() {
                self.sync_dir(&from_path, &to_path)?;
            } else {
                self.install_file(&from_path, &to_path)?;
            }
        }

        self.retire_unlisted(to, &shipped)
    }

    /// ディレクトリが無ければ作る
    ///
    /// 同名のファイルがあるときは退避してから作る
    fn ensure_dir(&mut self, path: &Path) -> Result<(), UpdateError> {
        if path.is_dir() {
            return Ok(());
        }
        if path.symlink_metadata().is_ok() {
            self.move_aside(path)?;
        }
        fs::create_dir(path).map_err(|err| {
            crate::log_warn!("ディレクトリ作成失敗: {}: {err}", path.display());
            UpdateError::DirReplace
        })?;
        self.steps.push(Step::Created(path.to_path_buf()));
        Ok(())
    }

    /// 宛先に既にある同名のパスを退避してからコピーする
    fn install_file(&mut self, from: &Path, to: &Path) -> Result<(), UpdateError> {
        if to.symlink_metadata().is_ok() {
            self.move_aside(to)?;
        }
        // コピーが途中で失敗しても書きかけを巻き戻せるよう、先に記録する
        self.steps.push(Step::Created(to.to_path_buf()));
        fs::copy(from, to).map_err(|err| {
            crate::log_warn!(
                "ファイルコピー失敗: {} -> {}: {err}",
                from.display(),
                to.display()
            );
            UpdateError::FileCopy {
                name: to
                    .file_name()
                    .map_or_else(String::new, |value| value.to_string_lossy().into_owned()),
            }
        })?;
        Ok(())
    }

    /// 配布物に含まれない `dir` 直下のエントリを退避する
    fn retire_unlisted(
        &mut self,
        dir: &Path,
        shipped: &HashSet<String>,
    ) -> Result<(), UpdateError> {
        let entries = fs::read_dir(dir).map_err(|err| {
            crate::log_warn!("ディレクトリ列挙失敗: {}: {err}", dir.display());
            UpdateError::DirReplace
        })?;
        let mut stale = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|err| {
                crate::log_warn!("ディレクトリエントリ取得失敗: {err}");
                UpdateError::DirReplace
            })?;
            if !shipped.contains(&entry.file_name().to_string_lossy().to_lowercase()) {
                stale.push(entry.path());
            }
        }
        for path in stale {
            self.move_aside(&path)?;
        }
        Ok(())
    }

    /// パスを退避フォルダへ移して記録する
    fn move_aside(&mut self, path: &Path) -> Result<(), UpdateError> {
        let backup = self.backup_dir.join(self.next_backup_id.to_string());
        self.next_backup_id += 1;
        fs::rename(path, &backup).map_err(|err| {
            crate::log_warn!("退避に失敗: {}: {err}", path.display());
            UpdateError::FileReplace
        })?;
        self.steps.push(Step::Moved {
            original: path.to_path_buf(),
            backup,
        });
        Ok(())
    }
}

/// ファイルまたはディレクトリを削除する (存在しなければ何もしない)
fn remove_path(path: &Path) -> std::io::Result<()> {
    let metadata = match path.symlink_metadata() {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    use super::super::test_support::unique_name;
    use super::*;

    /// `data` を残したまま配布ファイルを上書きコピーできるようにすること
    #[test]
    fn copy_portable_payload_skips_data_and_overwrites_files() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-copy"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(source.join("nested")).expect("source nested");
        fs::create_dir_all(source.join(PORTABLE_DATA_DIR_NAME)).expect("source data");
        fs::create_dir_all(dest.join(PORTABLE_DATA_DIR_NAME)).expect("dest data");
        fs::write(source.join("ClipRefiner.exe"), b"new-exe").expect("source exe");
        fs::write(source.join("PORTABLE.txt"), b"new").expect("source readme");
        fs::write(source.join("nested").join("a.txt"), b"nested-new").expect("nested");
        fs::write(
            source
                .join(PORTABLE_DATA_DIR_NAME)
                .join("should-not-copy.toml"),
            b"nope",
        )
        .expect("source data file");
        fs::write(dest.join("ClipRefiner.exe"), b"old-exe").expect("dest exe");
        fs::write(
            dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml"),
            b"keep-me",
        )
        .expect("dest config");

        copy_portable_payload(&source, &dest).expect("copy");

        assert_eq!(fs::read(dest.join("ClipRefiner.exe")).unwrap(), b"new-exe");
        assert_eq!(fs::read(dest.join("PORTABLE.txt")).unwrap(), b"new");
        assert_eq!(
            fs::read(dest.join("nested").join("a.txt")).unwrap(),
            b"nested-new"
        );
        assert_eq!(
            fs::read_to_string(dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml")).unwrap(),
            "keep-me"
        );
        assert!(
            !dest
                .join(PORTABLE_DATA_DIR_NAME)
                .join("should-not-copy.toml")
                .exists()
        );
        // 成功したら退避フォルダを残さない
        assert!(!dest.join(BACKUP_DIR_NAME).exists());

        let _ = fs::remove_dir_all(&root);
    }

    /// 配布物のサブフォルダから新しい版に無いファイルを取り除き、直下の利用者ファイルは残すこと
    #[test]
    fn copy_portable_payload_prunes_stale_files_in_shipped_dirs_only() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-prune"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(source.join("docs")).expect("source docs");
        fs::create_dir_all(dest.join("docs").join("old-dir")).expect("dest docs");
        fs::create_dir_all(dest.join(PORTABLE_DATA_DIR_NAME)).expect("dest data");
        fs::write(source.join("ClipRefiner.exe"), b"new-exe").expect("source exe");
        fs::write(source.join("docs").join("index.html"), b"new-index").expect("source index");
        fs::write(dest.join("docs").join("index.html"), b"old-index").expect("dest index");
        fs::write(dest.join("docs").join("removed.html"), b"gone").expect("dest removed");
        fs::write(dest.join("docs").join("old-dir").join("x.txt"), b"gone").expect("dest nested");
        fs::write(dest.join("my-shortcut.lnk"), b"user").expect("user file");
        fs::write(
            dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml"),
            b"keep",
        )
        .expect("config");

        copy_portable_payload(&source, &dest).expect("copy");

        // 新しい版のファイルに差し替わり、無くなったファイルとフォルダは消える
        assert_eq!(
            fs::read(dest.join("docs").join("index.html")).unwrap(),
            b"new-index"
        );
        assert!(!dest.join("docs").join("removed.html").exists());
        assert!(!dest.join("docs").join("old-dir").exists());
        // 直下の利用者ファイルと `data` は触らない
        assert_eq!(fs::read(dest.join("my-shortcut.lnk")).unwrap(), b"user");
        assert_eq!(
            fs::read(dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml")).unwrap(),
            b"keep"
        );
        assert!(!dest.join(BACKUP_DIR_NAME).exists());

        let _ = fs::remove_dir_all(&root);
    }

    /// 途中で失敗したら新旧が混ざらず、差し替え前の状態へ戻すこと
    ///
    /// 展開元の 1 ファイルを排他ロックしてコピーを失敗させる
    #[test]
    fn copy_portable_payload_rolls_back_on_failure() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-rollback"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(source.join("docs")).expect("source docs");
        fs::create_dir_all(source.join("added-dir")).expect("source added dir");
        fs::create_dir_all(dest.join("docs")).expect("dest docs");
        fs::create_dir_all(dest.join(PORTABLE_DATA_DIR_NAME)).expect("dest data");
        fs::write(source.join("ClipRefiner.exe"), b"new-exe").expect("source exe");
        fs::write(source.join("added.txt"), b"added").expect("source added");
        fs::write(source.join("added-dir").join("a.txt"), b"added").expect("source added nested");
        fs::write(source.join("docs").join("index.html"), b"new-index").expect("source index");
        fs::write(source.join("docs").join("locked.html"), b"new-locked").expect("source locked");
        fs::write(dest.join("ClipRefiner.exe"), b"old-exe").expect("dest exe");
        fs::write(dest.join("docs").join("index.html"), b"old-index").expect("dest index");
        fs::write(dest.join("docs").join("locked.html"), b"old-locked").expect("dest locked");
        fs::write(dest.join("docs").join("removed.html"), b"old-removed").expect("dest removed");
        fs::write(
            dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml"),
            b"keep",
        )
        .expect("config");

        // 共有を許さず開いて、コピー元として読めない状態にする
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(source.join("docs").join("locked.html"))
            .expect("lock source file");
        let err = copy_portable_payload(&source, &dest).expect_err("copy must fail");
        drop(lock);

        assert!(matches!(err, UpdateError::FileCopy { .. }), "{err:?}");
        // 既存ファイルは元の内容のまま
        assert_eq!(fs::read(dest.join("ClipRefiner.exe")).unwrap(), b"old-exe");
        assert_eq!(
            fs::read(dest.join("docs").join("index.html")).unwrap(),
            b"old-index"
        );
        assert_eq!(
            fs::read(dest.join("docs").join("locked.html")).unwrap(),
            b"old-locked"
        );
        assert_eq!(
            fs::read(dest.join("docs").join("removed.html")).unwrap(),
            b"old-removed"
        );
        assert_eq!(
            fs::read(dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml")).unwrap(),
            b"keep"
        );
        // 新規に作ったものと退避フォルダは残らない
        assert!(!dest.join("added.txt").exists());
        assert!(!dest.join("added-dir").exists());
        assert!(!dest.join(BACKUP_DIR_NAME).exists());

        let _ = fs::remove_dir_all(&root);
    }

    /// 前回の中断で残った退避フォルダがあっても差し替えられること
    #[test]
    fn copy_portable_payload_discards_leftover_backup_dir() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-leftover"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(&source).expect("source");
        fs::create_dir_all(dest.join(BACKUP_DIR_NAME)).expect("leftover backup");
        fs::write(dest.join(BACKUP_DIR_NAME).join("0"), b"stale").expect("leftover file");
        fs::write(source.join("ClipRefiner.exe"), b"new-exe").expect("source exe");
        fs::write(dest.join("ClipRefiner.exe"), b"old-exe").expect("dest exe");

        copy_portable_payload(&source, &dest).expect("copy");

        assert_eq!(fs::read(dest.join("ClipRefiner.exe")).unwrap(), b"new-exe");
        assert!(!dest.join(BACKUP_DIR_NAME).exists());

        let _ = fs::remove_dir_all(&root);
    }

    /// 差し替え先が無いときはコピーしないようにすること
    #[test]
    fn copy_portable_payload_rejects_missing_dest() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-nodest"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("ClipRefiner.exe"), b"x").expect("exe");
        let dest = root.join("missing");

        let err = copy_portable_payload(&source, &dest).expect_err("dest missing");
        assert_eq!(err, UpdateError::DestPrepare);

        let _ = fs::remove_dir_all(&root);
    }

    /// 展開直下の exe を検出できるようにすること
    #[test]
    fn find_portable_exe_finds_case_insensitive_name() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-find"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("cliprefiner.EXE"), b"x").expect("exe");

        let found = find_portable_exe(&root).expect("found");
        let name = found.file_name().and_then(|value| value.to_str()).unwrap();
        assert!(name.eq_ignore_ascii_case(&portable_exe_name()));

        let _ = fs::remove_dir_all(&root);
    }
}
