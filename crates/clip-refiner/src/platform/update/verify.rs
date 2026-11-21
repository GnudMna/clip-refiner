use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

// ======================================================================
// 定数
// ======================================================================
/// SHA-256 ダイジェストの16進文字列長
const SHA256_HEX_LEN: usize = 64;

// ======================================================================
// パブリック関数
// ======================================================================
/// アセットの完全性文字列を正規化した SHA-256 16進へ変換する
///
/// GitHub Release Asset の `digest` (`sha256:<hex>`) と素の16進の両方を受け付ける
///
/// # Arguments
/// * `raw` - `digest` / `sha256` フィールドの生文字列
///
/// # Returns
/// * `Result<String, String>` - 小文字16進 64 桁、またはエラーメッセージ
pub(crate) fn parse_asset_digest(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("アセットの完全性情報 (digest) が空です".to_string());
    }

    let hex = trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("SHA256:"))
        .unwrap_or(trimmed)
        .trim();

    if hex.len() != SHA256_HEX_LEN || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "アセットの完全性情報 (digest) の形式が不正です: {trimmed}"
        ));
    }

    Ok(hex.to_ascii_lowercase())
}

/// URL からファイルへダウンロードし、サイズと SHA-256 を検証する
///
/// 検証に失敗した場合は保存済みファイルを削除する
///
/// # Arguments
/// * `url` - ダウンロード URL
/// * `dest` - 保存先パス
/// * `expected_size` - 期待バイト数 (`0` のときはサイズ検証をスキップ)
/// * `expected_sha256` - 期待する SHA-256 (小文字16進 64 桁)
///
/// # Returns
/// * `Result<(), String>` - 成功時 `Ok(())`、失敗時はユーザー向けメッセージ
pub(crate) fn download_and_verify(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let result = download_and_verify_inner(url, dest, expected_size, expected_sha256);
    if result.is_err() {
        let _ = fs::remove_file(dest);
    }
    result
}

// ======================================================================
// プライベート関数
// ======================================================================
/// ダウンロード本体 (失敗時の削除は呼び出し側)
fn download_and_verify_inner(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let mut response = ureq::get(url).call().map_err(|err| {
        crate::log_warn!("更新ダウンロード HTTP 失敗: {err}");
        match err {
            ureq::Error::StatusCode(status) => {
                format!("更新ファイルのダウンロードに失敗しました (HTTP {status})")
            }
            _ => "更新ファイルのダウンロードに失敗しました".to_string(),
        }
    })?;

    let mut reader = response.body_mut().as_reader();
    let mut file = File::create(dest).map_err(|err| {
        crate::log_warn!("更新ファイルの作成に失敗: {err}");
        "更新ファイルの保存に失敗しました".to_string()
    })?;

    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(|err| {
            crate::log_warn!("更新ファイルの読み取りに失敗: {err}");
            "更新ファイルのダウンロードに失敗しました".to_string()
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).map_err(|err| {
            crate::log_warn!("更新ファイルの書き込みに失敗: {err}");
            "更新ファイルの保存に失敗しました".to_string()
        })?;
        hasher.update(&buffer[..n]);
        written = written.saturating_add(n as u64);
    }
    file.flush().map_err(|err| {
        crate::log_warn!("更新ファイルの flush に失敗: {err}");
        "更新ファイルの保存に失敗しました".to_string()
    })?;
    // 以降は読み取り検証のみにする
    drop(file);

    if expected_size > 0 && written != expected_size {
        crate::log_warn!("更新ファイルサイズ不一致: expected={expected_size} actual={written}");
        return Err(format!(
            "更新ファイルのサイズが一致しません (期待 {expected_size} / 実際 {written})"
        ));
    }

    let actual_sha256 = to_hex(&hasher.finalize());
    if actual_sha256 != expected_sha256 {
        crate::log_warn!(
            "更新ファイル SHA-256 不一致: expected={expected_sha256} actual={actual_sha256}"
        );
        return Err(
            "更新ファイルの完全性検証 (SHA-256) に失敗しました。再ダウンロードしてください"
                .to_string(),
        );
    }

    Ok(())
}

/// バイト列を小文字16進文字列へ変換する
fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ローカルファイルのサイズと SHA-256 を検証する (テスト用)
    fn verify_file_integrity(
        path: &Path,
        expected_size: u64,
        expected_sha256: &str,
    ) -> Result<(), String> {
        let meta = fs::metadata(path).map_err(|err| err.to_string())?;
        let actual_size = meta.len();
        if expected_size > 0 && actual_size != expected_size {
            return Err(format!(
                "更新ファイルのサイズが一致しません (期待 {expected_size} / 実際 {actual_size})"
            ));
        }

        let actual_sha256 = sha256_hex_of_file(path)?;
        if actual_sha256 != expected_sha256 {
            return Err(
                "更新ファイルの完全性検証 (SHA-256) に失敗しました。再ダウンロードしてください"
                    .to_string(),
            );
        }
        Ok(())
    }

    /// ファイル全体の SHA-256 を16進で返す (テスト用)
    fn sha256_hex_of_file(path: &Path) -> Result<String, String> {
        let mut file = File::open(path).map_err(|err| err.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8 * 1024];
        loop {
            let n = file.read(&mut buffer).map_err(|err| err.to_string())?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(to_hex(&hasher.finalize()))
    }

    /// GitHub 形式の `sha256:` 付き digest を正規化できること
    #[test]
    fn parse_asset_digest_accepts_github_prefix() {
        let hex = "a".repeat(SHA256_HEX_LEN);
        let parsed = parse_asset_digest(&format!("sha256:{hex}")).expect("digest");
        assert_eq!(parsed, hex);
    }

    /// 素の16進と大文字プレフィックスを正規化できること
    #[test]
    fn parse_asset_digest_normalizes_hex_case() {
        let upper = "B".repeat(SHA256_HEX_LEN);
        let parsed = parse_asset_digest(&format!("SHA256:{upper}")).expect("digest");
        assert_eq!(parsed, "b".repeat(SHA256_HEX_LEN));

        let bare = parse_asset_digest(&upper).expect("bare");
        assert_eq!(bare, "b".repeat(SHA256_HEX_LEN));
    }

    /// 不正な digest は拒否すること
    #[test]
    fn parse_asset_digest_rejects_invalid() {
        assert!(parse_asset_digest("").is_err());
        assert!(parse_asset_digest("sha256:abcd").is_err());
        assert!(parse_asset_digest(&"g".repeat(SHA256_HEX_LEN)).is_err());
    }

    /// サイズとハッシュが一致すれば検証成功すること
    #[test]
    fn verify_file_integrity_accepts_matching_file() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-verify-ok-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let path = root.join("payload.bin");
        let payload = b"clip-refiner-integrity";
        fs::write(&path, payload).expect("write");

        let digest = sha256_hex_of_file(&path).expect("hash");
        verify_file_integrity(&path, payload.len() as u64, &digest).expect("verify");

        let _ = fs::remove_dir_all(&root);
    }

    /// サイズ不一致を検出すること
    #[test]
    fn verify_file_integrity_rejects_size_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-verify-size-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let path = root.join("payload.bin");
        fs::write(&path, b"abc").expect("write");
        let digest = sha256_hex_of_file(&path).expect("hash");

        let err = verify_file_integrity(&path, 999, &digest).expect_err("size");
        assert!(err.contains("サイズ"));

        let _ = fs::remove_dir_all(&root);
    }

    /// SHA-256 不一致を検出すること
    #[test]
    fn verify_file_integrity_rejects_hash_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "clip-refiner-update-verify-hash-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let path = root.join("payload.bin");
        fs::write(&path, b"abc").expect("write");

        let wrong = "0".repeat(SHA256_HEX_LEN);
        let err = verify_file_integrity(&path, 3, &wrong).expect_err("hash");
        assert!(err.contains("SHA-256"));

        let _ = fs::remove_dir_all(&root);
    }
}
