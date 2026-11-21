use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use super::error::UpdateError;

use sha2::{Digest, Sha256};

// ======================================================================
// 定数
// ======================================================================
/// SHA-256 ダイジェストの16進文字列長
const SHA256_HEX_LEN: usize = 64;
/// 更新アセットダウンロードの絶対サイズ上限 (512 MiB)
///
/// MSI / ポータブル ZIP の想定サイズを大きく上回る応答でディスクを圧迫しないための上限
pub(crate) const MAX_UPDATE_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

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
/// * `Result<String, UpdateError>` - 小文字16進 64 桁、またはエラー
pub(crate) fn parse_asset_digest(raw: &str) -> Result<String, UpdateError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(UpdateError::DigestEmpty);
    }

    let hex = trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("SHA256:"))
        .unwrap_or(trimmed)
        .trim();

    if hex.len() != SHA256_HEX_LEN || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(UpdateError::DigestInvalid {
            raw: trimmed.to_string(),
        });
    }

    Ok(hex.to_ascii_lowercase())
}

/// アセットの宣言サイズがダウンロード可能な範囲か検証する
///
/// # Arguments
/// * `expected_size` - version API が報告するバイト数
///
/// # Returns
/// * `Result<(), UpdateError>` - 許容範囲なら `Ok(())`
pub(crate) fn validate_declared_download_size(expected_size: u64) -> Result<(), UpdateError> {
    if expected_size == 0 {
        return Err(UpdateError::SizeZero);
    }
    if expected_size > MAX_UPDATE_DOWNLOAD_BYTES {
        crate::log_warn!(
            "更新アセットサイズが絶対上限を超過: size={expected_size} max={MAX_UPDATE_DOWNLOAD_BYTES}"
        );
        return Err(UpdateError::SizeExceedsMax {
            max: MAX_UPDATE_DOWNLOAD_BYTES,
        });
    }
    Ok(())
}

/// 指定キーで URL からファイルへダウンロードし、サイズと SHA-256 を検証する
///
/// 期待サイズ超過または絶対上限超過の時点で読み取りを打ち切る
/// 検証に失敗した場合は保存済みファイルを削除する
///
/// # Arguments
/// * `url` - ダウンロード URL
/// * `dest` - 保存先パス
/// * `expected_size` - 期待バイト数 (0 は拒否)
/// * `expected_sha256` - 期待する SHA-256 (小文字16進 64 桁)
/// * `download_key` - bridge ダウンロードキー (`None` なら無認証)
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`
pub(crate) fn download_and_verify_with_key(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha256: &str,
    download_key: Option<&str>,
) -> Result<(), UpdateError> {
    let result = download_and_verify_inner(url, dest, expected_size, expected_sha256, download_key);
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
    download_key: Option<&str>,
) -> Result<(), UpdateError> {
    validate_declared_download_size(expected_size)?;

    let mut request = ureq::get(url);
    if let Some(key) = download_key {
        request = request.header("Authorization", &format!("Bearer {key}"));
    }
    let mut response = request.call().map_err(|err| {
        crate::log_warn!("更新ダウンロード HTTP 失敗: {err}");
        match err {
            ureq::Error::StatusCode(401) => UpdateError::DownloadUnauthorized,
            ureq::Error::StatusCode(status) => UpdateError::DownloadHttp { status },
            _ => UpdateError::DownloadFailed,
        }
    })?;

    let mut reader = response.body_mut().as_reader();
    let mut file = File::create(dest).map_err(|err| {
        crate::log_warn!("更新ファイルの作成に失敗: {err}");
        UpdateError::SaveFailed
    })?;

    let mut hasher = Sha256::new();
    let written = copy_and_hash_limited(&mut reader, &mut file, &mut hasher, expected_size)?;
    file.flush().map_err(|err| {
        crate::log_warn!("更新ファイルの flush に失敗: {err}");
        UpdateError::SaveFailed
    })?;
    // 以降は読み取り検証のみにする
    drop(file);

    if written != expected_size {
        crate::log_warn!("更新ファイルサイズ不一致: expected={expected_size} actual={written}");
        return Err(UpdateError::SizeMismatch {
            expected: expected_size,
            actual: written,
        });
    }

    let actual_sha256 = to_hex(&hasher.finalize());
    if actual_sha256 != expected_sha256 {
        crate::log_warn!(
            "更新ファイル SHA-256 不一致: expected={expected_sha256} actual={actual_sha256}"
        );
        return Err(UpdateError::IntegrityFailed);
    }

    Ok(())
}

/// Reader から Writer へコピーしつつ SHA-256 を更新し、上限超過で打ち切る
///
/// 次チャンクを書くと `cap` を超える場合は書き込まずにエラーを返す
///
/// # Arguments
/// * `reader` - 入力
/// * `writer` - 出力
/// * `hasher` - 累積用 SHA-256
/// * `cap` - 許容する最大バイト数
///
/// # Returns
/// * `Result<u64, UpdateError>` - 書き込んだバイト数
fn copy_and_hash_limited<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    hasher: &mut Sha256,
    cap: u64,
) -> Result<u64, UpdateError> {
    let mut written = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(|err| {
            crate::log_warn!("更新ファイルの読み取りに失敗: {err}");
            UpdateError::DownloadFailed
        })?;
        if n == 0 {
            break;
        }

        let n_u64 = n as u64;
        let Some(next) = written.checked_add(n_u64) else {
            crate::log_warn!("更新ダウンロード累積サイズがオーバーフロー");
            return Err(UpdateError::SizeCapOverflow);
        };
        if next > cap {
            crate::log_warn!(
                "更新ダウンロードをサイズ上限で打ち切り: written={written} chunk={n_u64} cap={cap}"
            );
            return Err(UpdateError::SizeMismatchTruncated { expected: cap });
        }

        writer.write_all(&buffer[..n]).map_err(|err| {
            crate::log_warn!("更新ファイルの書き込みに失敗: {err}");
            UpdateError::SaveFailed
        })?;
        hasher.update(&buffer[..n]);
        written = next;
    }
    Ok(written)
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
    use std::io::Cursor;

    use super::*;

    /// ローカルファイルのサイズと SHA-256 を検証する (テスト用)
    fn verify_file_integrity(
        path: &Path,
        expected_size: u64,
        expected_sha256: &str,
    ) -> Result<(), UpdateError> {
        let meta = fs::metadata(path).map_err(|err| {
            crate::log_warn!("更新ファイルのメタデータ取得に失敗: {err}");
            UpdateError::SaveFailed
        })?;
        let actual_size = meta.len();
        if expected_size > 0 && actual_size != expected_size {
            return Err(UpdateError::SizeMismatch {
                expected: expected_size,
                actual: actual_size,
            });
        }

        let actual_sha256 = sha256_hex_of_file(path)?;
        if actual_sha256 != expected_sha256 {
            return Err(UpdateError::IntegrityFailed);
        }
        Ok(())
    }

    /// ファイル全体の SHA-256 を16進で返す (テスト用)
    fn sha256_hex_of_file(path: &Path) -> Result<String, UpdateError> {
        let mut file = File::open(path).map_err(|err| {
            crate::log_warn!("更新ファイルのオープンに失敗: {err}");
            UpdateError::SaveFailed
        })?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8 * 1024];
        loop {
            let n = file.read(&mut buffer).map_err(|err| {
                crate::log_warn!("更新ファイルの読み取りに失敗: {err}");
                UpdateError::DownloadFailed
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(to_hex(&hasher.finalize()))
    }

    /// GitHub 形式の `sha256:` 付き digest を正規化できるようにすること
    #[test]
    fn parse_asset_digest_accepts_github_prefix() {
        let hex = "a".repeat(SHA256_HEX_LEN);
        let parsed = parse_asset_digest(&format!("sha256:{hex}")).expect("digest");
        assert_eq!(parsed, hex);
    }

    /// 素の16進と大文字プレフィックスを正規化できるようにすること
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

    /// 宣言サイズ 0 と絶対上限超過を拒否すること
    #[test]
    fn validate_declared_download_size_rejects_zero_and_over_max() {
        assert!(validate_declared_download_size(0).is_err());
        assert!(validate_declared_download_size(MAX_UPDATE_DOWNLOAD_BYTES).is_ok());
        assert!(validate_declared_download_size(MAX_UPDATE_DOWNLOAD_BYTES + 1).is_err());
    }

    /// 上限ちょうどの入力はコピーできるようにすること
    #[test]
    fn copy_and_hash_limited_accepts_exact_cap() {
        let payload = b"exact-cap-payload";
        let mut reader = Cursor::new(payload.as_slice());
        let mut out = Vec::new();
        let mut hasher = Sha256::new();

        let written =
            copy_and_hash_limited(&mut reader, &mut out, &mut hasher, payload.len() as u64)
                .expect("copy");
        assert_eq!(written, payload.len() as u64);
        assert_eq!(out, payload);
        assert_eq!(to_hex(&hasher.finalize()), to_hex(&Sha256::digest(payload)));
    }

    /// 上限を超える応答は書き込み前に打ち切るようにすること
    #[test]
    fn copy_and_hash_limited_aborts_before_writing_overflow() {
        let payload = b"0123456789";
        let mut reader = Cursor::new(payload.as_slice());
        let mut out = Vec::new();
        let mut hasher = Sha256::new();

        let err = copy_and_hash_limited(&mut reader, &mut out, &mut hasher, 4).expect_err("cap");
        assert!(err.to_string().contains("サイズ"));
        // 超過チャンクは書かない (単一 read で全体が届く場合は 0 バイトのまま)
        assert!(out.is_empty());
    }

    /// 複数チャンクでも累積が上限を超えた時点で打ち切るようにすること
    #[test]
    fn copy_and_hash_limited_aborts_after_partial_write() {
        // 1 回の `read` で最大 `chunk` バイトだけ返す Reader
        struct Chunked<'a> {
            data: &'a [u8],
            pos: usize,
            chunk: usize,
        }

        impl Read for Chunked<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let remain = self.data.len() - self.pos;
                let n = remain.min(self.chunk).min(buf.len());
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let payload = b"abcdefgh";
        let mut reader = Chunked {
            data: payload,
            pos: 0,
            chunk: 3,
        };
        let mut out = Vec::new();
        let mut hasher = Sha256::new();

        // 3 + 3 で 6 まで書き、次の 2 バイトで cap=6 を超えて打ち切る
        let err = copy_and_hash_limited(&mut reader, &mut out, &mut hasher, 6).expect_err("cap");
        assert!(err.to_string().contains("サイズ"));
        assert_eq!(out, b"abcdef");
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
        assert!(err.to_string().contains("サイズ"));

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
        assert!(err.to_string().contains("SHA-256"));

        let _ = fs::remove_dir_all(&root);
    }
}
