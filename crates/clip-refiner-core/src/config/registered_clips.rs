use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use super::atomic_io::write_bytes_atomically;
use super::clip_store_key::{clip_store_key, ensure_clip_store_key};
use super::paths::get_config_dir;
use super::permissions::restrict_private_file_permissions;
use super::registered_images::migrate_plain_image_to_encrypted;
use super::types::RegisteredClip;
use crate::consts;
use crate::error::ConfigError;
use crate::security::{
    SecretString, decrypt_bytes, encrypt_bytes, format_public_snippet, is_within_clipboard_limit,
    secret_from,
};

type Result<T> = std::result::Result<T, ConfigError>;

fn clips_err(msg: impl Into<String>) -> ConfigError {
    ConfigError::RegisteredClipsSave(msg.into())
}

// ======================================================================
// 定数
// ======================================================================
const REGISTERED_CLIPS_FILE: &str = "registered-clips.dat";
const REGISTERED_CLIPS_BACKUP_FILE: &str = "registered-clips.dat.bak";

/// 読み込みに失敗したまま、メモリ上の一覧と食い違っている可能性がある `registered-clips.dat` のパス
///
/// 該当パスへの保存は、退避コピーを作るまで削除・上書きしない
static UNLOADED_CLIPS_FILES: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

// ======================================================================
// ファイルペイロード
// ======================================================================
#[derive(Debug, Serialize, Deserialize)]
struct RegisteredClipsPayload {
    clips: Vec<RegisteredClip>,
}

/// 外側 blob 内のワイヤ形式 (新形式 / 平文 `text` レガシーを両対応)
#[derive(Debug, Deserialize)]
struct RegisteredClipWire {
    #[serde(default)]
    label: String,
    #[serde(default)]
    preview: String,
    /// レガシー: 平文本文
    #[serde(default)]
    text: Option<String>,
    /// 新形式: base64 暗号文
    #[serde(default)]
    text_ciphertext: Option<String>,
    #[serde(default)]
    image_file: Option<String>,
    #[serde(default)]
    template: bool,
    #[serde(default)]
    counter: u64,
}

#[derive(Debug, Deserialize)]
struct RegisteredClipsPayloadWire {
    clips: Vec<RegisteredClipWire>,
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 暗号化済み登録クリップファイルを読み込む
///
/// ファイルが無い場合は空配列を返す
/// 平文 `text` フィールドを持つ旧形式は暗号文へ移行する
/// ファイルがあるのに読み込めなかった場合は `registered-clips.dat.bak` へ退避し、
/// 次回以降の保存で空の一覧によるファイル削除や上書きが起きないようにする
/// 個別エントリの変換に失敗してスキップした場合も同様に退避し、
/// 次の保存で該当エントリが退避なしに失われないようにする
///
/// # Returns
/// * `Result<(Vec<RegisteredClip>, bool)>` - 登録クリップ一覧と、形式移行が発生したか
///
/// # Errors
/// 鍵取得、ファイル読み込み、復号、JSON 解析、本文暗号化のいずれかに失敗した場合
pub fn load_registered_clips() -> Result<(Vec<RegisteredClip>, bool)> {
    let result = read_registered_clips();
    let path = registered_clips_path();
    match (&result, path) {
        (Ok((_, _, false)), Ok(path)) => mark_clips_file_loaded(&path),
        (Ok((_, _, true)), Ok(path)) => quarantine_unloaded_clips_file(&path),
        (Err(_), Ok(path)) if path.is_file() => quarantine_unloaded_clips_file(&path),
        _ => {}
    }
    result.map(|(clips, migrated, _)| (clips, migrated))
}

/// ファイルを読み込み、`(一覧, 形式移行の有無, スキップしたエントリの有無)` を返す
fn read_registered_clips() -> Result<(Vec<RegisteredClip>, bool, bool)> {
    ensure_clip_store_key()?;

    let path = registered_clips_path()?;
    if !path.is_file() {
        return Ok((Vec::new(), false, false));
    }

    let bytes = fs::read(&path).map_err(|e| {
        clips_err(format!(
            "登録クリップファイルの読み込みに失敗: {}: {e}",
            path.display()
        ))
    })?;
    if bytes.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        return Err(clips_err(
            "登録クリップファイルのサイズが上限を超えています",
        ));
    }

    let key = clip_store_key()?;
    let json = decrypt_bytes(&key, &bytes)
        .map_err(|e| clips_err(format!("登録クリップファイルの復号に失敗: {e}")))?;
    let payload: RegisteredClipsPayloadWire = serde_json::from_slice(&json)
        .map_err(|e| clips_err(format!("登録クリップファイルの JSON 解析に失敗: {e}")))?;

    let mut migrated = false;
    let mut skipped = false;
    let mut clips = Vec::with_capacity(payload.clips.len());
    for wire in payload.clips {
        match registered_clip_from_wire(wire) {
            Ok((clip, entry_migrated)) => {
                migrated |= entry_migrated;
                clips.push(clip);
            }
            Err(err) => {
                tracing::warn!("登録クリップの読み込みをスキップ: {:?}", err);
                migrated = true;
                skipped = true;
            }
        }
    }
    Ok((clips, migrated, skipped))
}

/// 登録クリップ一覧を暗号化ファイルへ保存する
///
/// 空の場合はファイルを削除する
/// 平文本文はシリアライズしない (暗号文フィールドのみ)
/// 直前の読み込みに失敗したファイルは、空の一覧では削除せず、
/// 一覧がある場合も退避コピーを作ってから上書きする
///
/// # Arguments
/// * `clips` - 保存する登録クリップ一覧
///
/// # Returns
/// * `Result<()>` - 保存または削除に成功した場合は `Ok(())`
///
/// # Errors
/// 鍵取得、シリアライズ、暗号化、退避コピー、ファイル書き込みのいずれかに失敗した場合
pub fn save_registered_clips(clips: &[RegisteredClip]) -> Result<()> {
    ensure_clip_store_key()?;

    let path = registered_clips_path()?;
    let unloaded = is_clips_file_unloaded(&path);
    if clips.is_empty() {
        if unloaded {
            tracing::warn!("登録クリップファイルを読み込めていないため、削除せず残す");
        } else if path.is_file() {
            let _ = fs::remove_file(path);
        }
        return Ok(());
    }

    let json = serde_json::to_vec(&RegisteredClipsPayload {
        clips: clips.to_vec(),
    })
    .map_err(|e| clips_err(format!("登録クリップの JSON シリアライズに失敗: {e}")))?;
    if json.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        return Err(clips_err(
            "登録クリップのシリアライズサイズが上限を超えています",
        ));
    }

    let key = clip_store_key()?;
    let encrypted = encrypt_bytes(&key, &json)?;
    if encrypted.len() > consts::MAX_REGISTERED_CLIPS_FILE_BYTES {
        return Err(clips_err(
            "登録クリップファイルの暗号化サイズが上限を超えています",
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::CreateDir)?;
    }
    if unloaded {
        backup_registered_clips_file(&path)?;
    }
    write_bytes_atomically(&path, &encrypted).map_err(|e| {
        clips_err(format!(
            "登録クリップファイルの書き込みに失敗: {}: {e}",
            path.display()
        ))
    })?;
    mark_clips_file_loaded(&path);
    if let Err(e) = restrict_private_file_permissions(&path) {
        tracing::warn!("登録クリップファイルのパーミッション設定に失敗: {:?}", e);
    }
    Ok(())
}

/// レガシー `.png` 登録画像を暗号化 `.enc` へ移行する
///
/// # Arguments
/// * `clips` - 移行対象の登録クリップ一覧
///
/// # Returns
/// * `bool` - 1 件以上移行した場合は `true`
pub fn migrate_legacy_clip_images(clips: &mut [RegisteredClip]) -> bool {
    let mut migrated = false;
    for clip in clips {
        let Some(ref image_file) = clip.image_file else {
            continue;
        };
        if is_encrypted_image_filename(image_file) {
            continue;
        }
        match migrate_plain_image_to_encrypted(image_file) {
            Ok(new_file) => {
                clip.image_file = Some(new_file);
                migrated = true;
            }
            Err(err) => {
                tracing::warn!("登録画像の暗号化移行に失敗 ({}): {:?}", image_file, err);
                clip.image_file = None;
                migrated = true;
            }
        }
    }
    migrated
}

/// 平文本文を AEAD 暗号文と表示用プレビューへ変換する
///
/// # Arguments
/// * `text` - 暗号化する平文
///
/// # Returns
/// * `Result<(Vec<u8>, String)>` - `(text_ciphertext, preview)`
///
/// # Errors
/// 鍵取得または暗号化に失敗した場合
pub(crate) fn encrypt_registered_clip_text(text: &str) -> Result<(Vec<u8>, String)> {
    ensure_clip_store_key()?;
    let key = clip_store_key()?;
    let ciphertext = encrypt_bytes(&key, text.as_bytes())?;
    let preview = format_public_snippet(text, consts::REGISTERED_CLIP_PREVIEW_MAX_CHARS);
    Ok((ciphertext, preview))
}

/// 登録クリップ本文の暗号文を復号する
///
/// # Arguments
/// * `ciphertext` - `encrypt_bytes` 形式の暗号文
///
/// # Returns
/// * `Result<SecretString>` - 復号した平文 (破棄時にゼロ化)
///
/// # Errors
/// 鍵取得、復号、UTF-8 解釈のいずれかに失敗した場合
pub(crate) fn decrypt_registered_clip_text(ciphertext: &[u8]) -> Result<SecretString> {
    let key = clip_store_key()?;
    let plain = decrypt_bytes(&key, ciphertext)
        .map_err(|e| clips_err(format!("登録クリップ本文の復号に失敗: {e}")))?;
    let text = String::from_utf8(plain)
        .map_err(|e| clips_err(format!("登録クリップ本文が UTF-8 ではありません: {e}")))?;
    Ok(secret_from(text))
}

/// 平文ラベル・本文からメモリ用の登録クリップを組み立てる
///
/// # Arguments
/// * `label` - 表示ラベル (空なら正規化時に preview から補完)
/// * `text` - 平文本文
///
/// # Returns
/// * `Result<RegisteredClip>` - 暗号文を保持する登録クリップ
///
/// # Errors
/// 本文が空 / サイズ超過、または暗号化に失敗した場合
pub(crate) fn registered_clip_from_plain_text(label: String, text: &str) -> Result<RegisteredClip> {
    if text.trim().is_empty() {
        return Err(clips_err("登録クリップが空です"));
    }
    if !is_within_clipboard_limit(text) {
        return Err(clips_err("登録クリップのサイズが上限を超えています"));
    }
    let (text_ciphertext, preview) = encrypt_registered_clip_text(text)?;
    Ok(RegisteredClip {
        label,
        preview,
        text_ciphertext,
        image_file: None,
        template: false,
        counter: 0,
    })
}

// ======================================================================
// プライベート関数
// ======================================================================
fn registered_clips_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join(REGISTERED_CLIPS_FILE))
}

fn backup_path_of(path: &Path) -> PathBuf {
    path.with_file_name(REGISTERED_CLIPS_BACKUP_FILE)
}

/// 一覧と食い違う登録クリップファイルを未読込として記録し、`.bak` へ退避する
fn quarantine_unloaded_clips_file(path: &Path) {
    mark_clips_file_unloaded(path);
    if backup_registered_clips_file(path).is_ok() {
        tracing::warn!(
            "読み込めなかった登録クリップを含むファイルを退避した: {}",
            backup_path_of(path).display()
        );
    }
}

/// 読み込めなかった登録クリップファイルを `registered-clips.dat.bak` へ退避する
///
/// 存在しないファイルは退避不要なので成功扱いにする
fn backup_registered_clips_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let backup_path = backup_path_of(path);
    fs::copy(path, &backup_path).map_err(|e| {
        tracing::warn!("登録クリップファイルの退避に失敗: {:?}", e);
        clips_err(format!(
            "登録クリップファイルの退避に失敗: {}: {e}",
            backup_path.display()
        ))
    })?;
    if let Err(e) = restrict_private_file_permissions(&backup_path) {
        tracing::warn!(
            "登録クリップ退避ファイルのパーミッション設定に失敗: {:?}",
            e
        );
    }
    Ok(())
}

fn unloaded_clips_files() -> MutexGuard<'static, Option<HashSet<PathBuf>>> {
    UNLOADED_CLIPS_FILES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn mark_clips_file_unloaded(path: &Path) {
    unloaded_clips_files()
        .get_or_insert_with(HashSet::new)
        .insert(path.to_path_buf());
}

fn mark_clips_file_loaded(path: &Path) {
    if let Some(paths) = unloaded_clips_files().as_mut() {
        paths.remove(path);
    }
}

fn is_clips_file_unloaded(path: &Path) -> bool {
    unloaded_clips_files()
        .as_ref()
        .is_some_and(|paths| paths.contains(path))
}

fn is_encrypted_image_filename(filename: &str) -> bool {
    Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("enc"))
}

/// ワイヤ形式をランタイム用 `RegisteredClip` へ変換する
fn registered_clip_from_wire(wire: RegisteredClipWire) -> Result<(RegisteredClip, bool)> {
    if wire.image_file.is_some() {
        return Ok((
            RegisteredClip {
                label: wire.label,
                preview: wire.preview,
                text_ciphertext: Vec::new(),
                image_file: wire.image_file,
                template: false,
                counter: 0,
            },
            false,
        ));
    }

    if let Some(encoded) = wire.text_ciphertext.filter(|s| !s.is_empty()) {
        let text_ciphertext = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            encoded.as_bytes(),
        )
        .map_err(|e| clips_err(format!("登録クリップ本文暗号文の base64 復号に失敗: {e}")))?;

        let mut clip = RegisteredClip {
            label: wire.label,
            preview: wire.preview,
            text_ciphertext,
            image_file: None,
            template: wire.template,
            counter: wire.counter,
        };
        let mut migrated = false;
        if clip.preview.is_empty()
            && let Ok(text) = decrypt_registered_clip_text(&clip.text_ciphertext)
        {
            clip.preview = format_public_snippet(&text, consts::REGISTERED_CLIP_PREVIEW_MAX_CHARS);
            migrated = true;
        }
        return Ok((clip, migrated));
    }

    if let Some(text) = wire.text {
        let clip = registered_clip_from_plain_text(wire.label, &text)?;
        return Ok((clip, true));
    }

    Err(clips_err("登録クリップに本文がありません"))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::encrypt_bytes;

    /// 暗号化ファイルの往復で平文 JSON がディスクに残らないようにすること
    #[test]
    fn save_and_load_do_not_persist_plaintext() {
        crate::test_helpers::with_temp_config_dir(|| {
            let clip =
                registered_clip_from_plain_text("secret-label".into(), "secret-body").expect("enc");
            let clips = vec![clip.clone()];

            save_registered_clips(&clips).expect("save");
            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(!migrated);
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].label, "secret-label");
            assert!(!loaded[0].text_ciphertext.is_empty());
            let plain = decrypt_registered_clip_text(&loaded[0].text_ciphertext).expect("dec");
            assert_eq!(plain.as_str(), "secret-body");

            let path = registered_clips_path().expect("path");
            let bytes = fs::read(path).expect("read");
            let content = String::from_utf8_lossy(&bytes);
            assert!(!content.contains("secret-body"));
            assert!(!content.contains("secret-label"));
        });
    }

    /// テンプレート指定を保存・読み込みで維持し、未指定の旧データは無効として読むこと
    #[test]
    fn save_and_load_preserve_template_flag() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut template = registered_clip_from_plain_text("t".into(), "{date}").expect("enc");
            template.template = true;
            let plain = registered_clip_from_plain_text("p".into(), "body").expect("enc");

            save_registered_clips(&[template, plain]).expect("save");
            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(!migrated);
            assert!(loaded[0].template);
            assert!(!loaded[1].template);

            // 無効なクリップは `template` キーを書かない (旧版でも読める形を保つ)
            let key = clip_store_key().expect("key");
            let bytes = fs::read(registered_clips_path().expect("path")).expect("read");
            let inner = String::from_utf8(decrypt_bytes(&key, &bytes).expect("dec")).expect("utf8");
            assert_eq!(inner.matches("\"template\"").count(), 1);
        });
    }

    /// 連番を保存・読み込みで維持し、`counter` キーの無い旧データは未使用 (0) として読むこと
    #[test]
    fn save_and_load_preserve_counter() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut used = registered_clip_from_plain_text("u".into(), "{counter}").expect("enc");
            used.counter = 12;
            let fresh = registered_clip_from_plain_text("f".into(), "body").expect("enc");

            save_registered_clips(&[used, fresh]).expect("save");
            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(!migrated);
            assert_eq!(loaded[0].counter, 12);
            assert_eq!(loaded[1].counter, 0);

            // `counter` キーの無い旧形式 (ここでは `template` のみ) も 0 として読める
            let legacy_json =
                br#"{"clips":[{"label":"old","text_ciphertext":"","template":true}]}"#;
            let wire: RegisteredClipsPayloadWire =
                serde_json::from_slice(legacy_json).expect("wire");
            assert_eq!(wire.clips[0].counter, 0);
        });
    }

    /// 平文 `text` を持つ旧形式を暗号文形式へ移行すること
    #[test]
    fn load_migrates_legacy_plaintext_text_field() {
        crate::test_helpers::with_temp_config_dir(|| {
            ensure_clip_store_key().expect("key");
            let key = clip_store_key().expect("key");
            let legacy_json = br#"{"clips":[{"label":"legacy","text":"plain-secret"}]}"#;
            let encrypted = encrypt_bytes(&key, legacy_json).expect("outer enc");
            let path = registered_clips_path().expect("path");
            fs::write(&path, encrypted).expect("write");

            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(migrated);
            assert_eq!(loaded.len(), 1);
            assert!(!loaded[0].text_ciphertext.is_empty());
            let plain = decrypt_registered_clip_text(&loaded[0].text_ciphertext).expect("dec");
            assert_eq!(plain.as_str(), "plain-secret");
            assert!(!loaded[0].preview.is_empty());

            // 再保存後の内側 JSON に平文本文キーが無いこと
            save_registered_clips(&loaded).expect("resave");
            let bytes = fs::read(path).expect("read");
            let inner = decrypt_bytes(&key, &bytes).expect("outer dec");
            let inner_str = String::from_utf8(inner).expect("utf8");
            assert!(!inner_str.contains("\"text\":"));
            assert!(inner_str.contains("text_ciphertext"));
            // preview には表示用断片が残るが、平文本文フィールドは無い
            assert!(inner_str.contains("\"preview\""));
        });
    }

    /// 復号できない `registered-clips.dat` を `.bak` へ退避し、空の一覧で保存しても削除しないこと
    #[test]
    fn load_failure_backs_up_and_empty_save_keeps_file() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = registered_clips_path().expect("path");
            ensure_clip_store_key().expect("key");
            fs::write(&path, b"not-a-valid-ciphertext").expect("write");

            assert!(load_registered_clips().is_err());
            assert_eq!(
                fs::read(backup_path_of(&path)).expect("backup"),
                b"not-a-valid-ciphertext"
            );

            save_registered_clips(&[]).expect("save empty");
            assert_eq!(
                fs::read(&path).expect("kept"),
                b"not-a-valid-ciphertext",
                "読み込み失敗後の空保存でファイルが残ること"
            );
        });
    }

    /// 読み込みに失敗した後に一覧を保存すると上書きし、以降の空保存では通常どおり削除すること
    #[test]
    fn save_after_load_failure_overwrites_then_resumes_normal_delete() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = registered_clips_path().expect("path");
            ensure_clip_store_key().expect("key");
            fs::write(&path, b"broken").expect("write");
            assert!(load_registered_clips().is_err());

            let clip = registered_clip_from_plain_text("l".into(), "body").expect("enc");
            save_registered_clips(&[clip]).expect("save");
            assert_eq!(fs::read(backup_path_of(&path)).expect("backup"), b"broken");
            assert_eq!(load_registered_clips().expect("load").0.len(), 1);

            save_registered_clips(&[]).expect("save empty");
            assert!(!path.exists(), "復旧後の空保存で削除されること");
        });
    }

    /// 退避コピーを作れない場合は、読み込めなかったファイルを上書きしないこと
    ///
    /// `.bak` の位置にディレクトリを置いてコピー失敗を再現する
    #[test]
    fn save_aborts_when_backup_of_unloaded_file_fails() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = registered_clips_path().expect("path");
            ensure_clip_store_key().expect("key");
            fs::write(&path, b"broken").expect("write");
            fs::create_dir_all(backup_path_of(&path)).expect("backup dir");
            assert!(load_registered_clips().is_err());

            let clip = registered_clip_from_plain_text("l".into(), "body").expect("enc");
            assert!(save_registered_clips(&[clip]).is_err());
            assert_eq!(fs::read(&path).expect("kept"), b"broken");
        });
    }

    /// 個別エントリの変換に失敗してスキップした場合も `.bak` へ退避し、空の一覧では削除しないこと
    ///
    /// 保存すると退避コピーを残したまま、スキップしたエントリのない状態で上書きする
    #[test]
    fn skipped_entry_backs_up_and_protects_file_until_save() {
        crate::test_helpers::with_temp_config_dir(|| {
            ensure_clip_store_key().expect("key");
            let key = clip_store_key().expect("key");
            let json =
                br#"{"clips":[{"label":"ok","text":"body"},{"label":"empty","text":"   "}]}"#;
            let encrypted = encrypt_bytes(&key, json).expect("outer enc");
            let path = registered_clips_path().expect("path");
            fs::write(&path, &encrypted).expect("write");

            let (loaded, migrated) = load_registered_clips().expect("load");
            assert!(migrated);
            assert_eq!(loaded.len(), 1);
            assert_eq!(fs::read(backup_path_of(&path)).expect("backup"), encrypted);
            assert!(is_clips_file_unloaded(&path));

            // スキップしたエントリが残るファイルは、空の一覧で削除しない
            save_registered_clips(&[]).expect("save empty");
            assert_eq!(fs::read(&path).expect("kept"), encrypted);

            // 保存後は退避コピーを残したまま通常状態へ戻る
            save_registered_clips(&loaded).expect("save");
            assert_eq!(fs::read(backup_path_of(&path)).expect("backup"), encrypted);
            assert!(!is_clips_file_unloaded(&path));
            let (reloaded, migrated) = load_registered_clips().expect("reload");
            assert!(!migrated);
            assert_eq!(reloaded.len(), 1);
        });
    }

    /// ファイルが存在しない場合は退避もマークもせず、空保存は何もしないこと
    #[test]
    fn missing_file_is_not_treated_as_load_failure() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = registered_clips_path().expect("path");
            let (clips, migrated) = load_registered_clips().expect("load");
            assert!(clips.is_empty() && !migrated);
            assert!(!backup_path_of(&path).exists());
            assert!(!is_clips_file_unloaded(&path));
        });
    }

    /// `Debug` が暗号文バイト列を展開しないようにすること
    #[test]
    fn registered_clip_debug_masks_ciphertext() {
        crate::test_helpers::with_temp_config_dir(|| {
            let clip = registered_clip_from_plain_text("lbl".into(), "secret-body").expect("enc");
            let debug = format!("{clip:?}");
            assert!(debug.contains("text_ciphertext: \"...\""));
            assert!(!debug.contains(&format!("{:?}", clip.text_ciphertext)));
        });
    }
}
