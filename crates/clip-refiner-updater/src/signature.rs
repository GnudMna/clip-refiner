//! minisign による更新アセットの署名検証
//!
//! リリース時に秘密鍵で各アセットの `<asset>.minisig` を作り、クライアントはビルド時に埋め込んだ公開鍵で検証する
//! 信頼コメントにバージョンとアセット名を入れて署名するため、署名済みの別アセット (古い版など) へのすり替えも拒否する

use std::fs::File;
use std::io::Read;
use std::path::Path;

use minisign_verify::{PublicKey, Signature};

use super::error::UpdateError;
use super::http;

include!(concat!(env!("OUT_DIR"), "/update_signing_keys.rs"));

// ======================================================================
// 定数
// ======================================================================
/// 署名ファイルの拡張子 (アセット名の末尾に付ける)
pub(crate) const SIGNATURE_SUFFIX: &str = ".minisig";
/// 署名ファイルの最大バイト数 (minisign の署名は数百バイト)
const MAX_SIGNATURE_BYTES: u64 = 4 * 1024;
/// 信頼コメントの接頭辞
const TRUSTED_COMMENT_PREFIX: &str = "clip-refiner-update";
/// 検証時の読み取りチャンクサイズ
const READ_CHUNK_BYTES: usize = 64 * 1024;

// ======================================================================
// パブリック関数
// ======================================================================
/// ビルド時に埋め込んだ更新署名の公開鍵 (Base64) を返す
///
/// # Returns
/// * `&'static [&'static str]` - `keys/update-signing/*.pub` の公開鍵一覧
pub(crate) fn embedded_signing_keys() -> &'static [&'static str] {
    UPDATE_SIGNING_KEYS
}

/// アセットに対応する署名ファイル名を返す
///
/// # Arguments
/// * `asset_name` - アセット名
///
/// # Returns
/// * `String` - `<asset_name>.minisig`
pub(crate) fn signature_asset_name(asset_name: &str) -> String {
    format!("{asset_name}{SIGNATURE_SUFFIX}")
}

/// 署名の信頼コメントとして期待する文字列を返す
///
/// リリース時の署名 (`release.ps1`) と同じ形式にする
///
/// # Arguments
/// * `version` - 更新先バージョン (`SemVer`、`v` なし)
/// * `asset_name` - アセット名
///
/// # Returns
/// * `String` - `clip-refiner-update version=<version> file=<asset_name>`
pub(crate) fn expected_trusted_comment(version: &str, asset_name: &str) -> String {
    format!("{TRUSTED_COMMENT_PREFIX} version={version} file={asset_name}")
}

/// 署名ファイルをダウンロードして文字列で返す
///
/// # Arguments
/// * `url` - 署名ファイルの URL
///
/// # Returns
/// * `Result<String, UpdateError>` - 署名ファイルの内容
pub(crate) fn download_signature(url: &str) -> Result<String, UpdateError> {
    let mut response = http::api_agent(url).get(url).call().map_err(|err| {
        tracing::warn!("更新署名の取得 HTTP 失敗: {err}");
        match err {
            ureq::Error::StatusCode(status) => UpdateError::SignatureDownloadHttp { status },
            _ => UpdateError::SignatureDownloadFailed,
        }
    })?;

    response
        .body_mut()
        .with_config()
        .limit(MAX_SIGNATURE_BYTES)
        .read_to_string()
        .map_err(|err| {
            tracing::warn!("更新署名の読み取りに失敗: {err}");
            UpdateError::SignatureDownloadFailed
        })
}

/// ファイルの minisign 署名と信頼コメントを検証する
///
/// 署名鍵 ID が一致する公開鍵で署名を検証し、信頼コメントが更新先バージョン・アセット名と一致することを確認する
///
/// # Arguments
/// * `path` - 検証するファイル
/// * `signature_text` - minisign 署名ファイルの内容
/// * `version` - 更新先バージョン (`SemVer`、`v` なし)
/// * `asset_name` - アセット名
/// * `trusted_keys` - 信頼する公開鍵 (Base64)
///
/// # Returns
/// * `Result<(), UpdateError>` - 検証に成功した場合は `Ok(())`
pub(crate) fn verify_file_signature(
    path: &Path,
    signature_text: &str,
    version: &str,
    asset_name: &str,
    trusted_keys: &[&str],
) -> Result<(), UpdateError> {
    let keys = parse_trusted_keys(trusted_keys);
    if keys.is_empty() {
        tracing::warn!("更新署名の公開鍵がビルドに埋め込まれていない");
        return Err(UpdateError::SignatureKeyMissing);
    }

    let signature = Signature::decode(signature_text).map_err(|err| {
        tracing::warn!("更新署名の形式が不正: {err}");
        UpdateError::SignatureInvalid
    })?;

    let Some(mut verifier) = keys
        .iter()
        .find_map(|key| key.verify_stream(&signature).ok())
    else {
        tracing::warn!("更新署名の鍵が信頼する公開鍵に含まれない");
        return Err(UpdateError::SignatureUntrustedKey);
    };

    let mut file = File::open(path).map_err(|err| {
        tracing::warn!("署名検証対象を開けない: {err}");
        UpdateError::SignatureInvalid
    })?;
    let mut buffer = vec![0u8; READ_CHUNK_BYTES];
    loop {
        let n = file.read(&mut buffer).map_err(|err| {
            tracing::warn!("署名検証対象の読み取りに失敗: {err}");
            UpdateError::SignatureInvalid
        })?;
        if n == 0 {
            break;
        }
        verifier.update(&buffer[..n]);
    }
    verifier.finalize().map_err(|err| {
        tracing::warn!("更新署名の検証に失敗: {err}");
        UpdateError::SignatureInvalid
    })?;

    // 信頼コメントはグローバル署名で保護されるため、署名検証の成功後に照合する
    let expected = expected_trusted_comment(version, asset_name);
    if signature.trusted_comment() != expected {
        tracing::warn!(
            "更新署名の信頼コメント不一致: expected={expected} actual={}",
            signature.trusted_comment()
        );
        return Err(UpdateError::SignatureMetadataMismatch);
    }

    Ok(())
}

// ======================================================================
// プライベート関数
// ======================================================================
/// Base64 の公開鍵一覧を解釈する (不正な鍵は警告して除外)
fn parse_trusted_keys(trusted_keys: &[&str]) -> Vec<PublicKey> {
    trusted_keys
        .iter()
        .filter_map(|raw| match PublicKey::from_base64(raw) {
            Ok(key) => Some(key),
            Err(err) => {
                tracing::warn!("更新署名の公開鍵を解釈できない: {err}");
                None
            }
        })
        .collect()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;

    use minisign::KeyPair;

    use super::*;

    /// テスト用の一時ファイルへ内容を書き、そのパスを返す
    fn write_temp_file(name: &str, body: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "clip-refiner-signature-{}-{name}",
            std::process::id()
        ));
        fs::write(&path, body).expect("write temp file");
        path
    }

    /// 指定鍵と信頼コメントで署名ファイルの内容を作る
    fn sign(key_pair: &KeyPair, body: &[u8], trusted_comment: &str) -> String {
        minisign::sign(
            Some(&key_pair.pk),
            &key_pair.sk,
            Cursor::new(body),
            Some(trusted_comment),
            None,
        )
        .expect("sign")
        .into_string()
    }

    /// テスト用の鍵ペアを生成する
    fn key_pair() -> KeyPair {
        KeyPair::generate_unencrypted_keypair().expect("keypair")
    }

    /// 正しい鍵と信頼コメントの署名を受け入れること
    #[test]
    fn verify_file_signature_accepts_valid_signature() {
        let key_pair = key_pair();
        let public_key = key_pair.pk.to_base64();
        let body = b"valid-asset";
        let path = write_temp_file("valid", body);
        let signature = sign(&key_pair, body, &expected_trusted_comment("1.2.3", "a.msi"));

        let result = verify_file_signature(&path, &signature, "1.2.3", "a.msi", &[&public_key]);
        let _ = fs::remove_file(&path);
        assert_eq!(result, Ok(()));
    }

    /// 複数の公開鍵のうち署名鍵に一致するもので検証すること
    #[test]
    fn verify_file_signature_selects_matching_key() {
        let signer = key_pair();
        let other = key_pair();
        let body = b"rotated-key";
        let path = write_temp_file("rotated", body);
        let signature = sign(&signer, body, &expected_trusted_comment("1.2.3", "a.msi"));

        let keys = [other.pk.to_base64(), signer.pk.to_base64()];
        let keys = keys.iter().map(String::as_str).collect::<Vec<_>>();
        let result = verify_file_signature(&path, &signature, "1.2.3", "a.msi", &keys);
        let _ = fs::remove_file(&path);
        assert_eq!(result, Ok(()));
    }

    /// 署名後に内容が変わったファイルを拒否すること
    #[test]
    fn verify_file_signature_rejects_tampered_file() {
        let key_pair = key_pair();
        let public_key = key_pair.pk.to_base64();
        let path = write_temp_file("tampered", b"tampered-asset");
        let signature = sign(
            &key_pair,
            b"original-asset",
            &expected_trusted_comment("1.2.3", "a.msi"),
        );

        let result = verify_file_signature(&path, &signature, "1.2.3", "a.msi", &[&public_key]);
        let _ = fs::remove_file(&path);
        assert_eq!(result, Err(UpdateError::SignatureInvalid));
    }

    /// 信頼しない鍵の署名を拒否すること
    #[test]
    fn verify_file_signature_rejects_untrusted_key() {
        let signer = key_pair();
        let trusted = key_pair().pk.to_base64();
        let body = b"untrusted-asset";
        let path = write_temp_file("untrusted", body);
        let signature = sign(&signer, body, &expected_trusted_comment("1.2.3", "a.msi"));

        let result = verify_file_signature(&path, &signature, "1.2.3", "a.msi", &[&trusted]);
        let _ = fs::remove_file(&path);
        assert_eq!(result, Err(UpdateError::SignatureUntrustedKey));
    }

    /// 別バージョン・別アセット向けの署名を拒否すること
    #[test]
    fn verify_file_signature_rejects_other_version_or_asset() {
        let key_pair = key_pair();
        let public_key = key_pair.pk.to_base64();
        let body = b"old-asset";
        let path = write_temp_file("rollback", body);
        let signature = sign(&key_pair, body, &expected_trusted_comment("1.0.0", "a.msi"));

        // 古い版の正規署名を新しい版として渡されても拒否する
        let newer = verify_file_signature(&path, &signature, "9.9.9", "a.msi", &[&public_key]);
        // 同じ版の別アセット名でも拒否する
        let other_asset =
            verify_file_signature(&path, &signature, "1.0.0", "b.zip", &[&public_key]);
        let _ = fs::remove_file(&path);
        assert_eq!(newer, Err(UpdateError::SignatureMetadataMismatch));
        assert_eq!(other_asset, Err(UpdateError::SignatureMetadataMismatch));
    }

    /// 公開鍵が無いときは検証せずに失敗すること
    #[test]
    fn verify_file_signature_requires_trusted_key() {
        let path = write_temp_file("no-key", b"no-key");
        let result = verify_file_signature(&path, "", "1.2.3", "a.msi", &[]);
        let invalid_only = verify_file_signature(&path, "", "1.2.3", "a.msi", &["not-a-key"]);
        let _ = fs::remove_file(&path);
        assert_eq!(result, Err(UpdateError::SignatureKeyMissing));
        assert_eq!(invalid_only, Err(UpdateError::SignatureKeyMissing));
    }

    /// 形式が壊れた署名ファイルを拒否すること
    #[test]
    fn verify_file_signature_rejects_malformed_signature() {
        let public_key = key_pair().pk.to_base64();
        let path = write_temp_file("malformed", b"malformed");
        let result = verify_file_signature(&path, "garbage", "1.2.3", "a.msi", &[&public_key]);
        let _ = fs::remove_file(&path);
        assert_eq!(result, Err(UpdateError::SignatureInvalid));
    }

    /// 埋め込み公開鍵がすべて解釈できること
    #[test]
    fn embedded_signing_keys_are_valid() {
        let keys = embedded_signing_keys();
        assert_eq!(parse_trusted_keys(keys).len(), keys.len());
    }

    /// 署名ファイル名と信頼コメントの形式を固定すること
    #[test]
    fn signature_names_follow_release_convention() {
        assert_eq!(signature_asset_name("a.msi"), "a.msi.minisig");
        assert_eq!(
            expected_trusted_comment("1.2.3", "a.msi"),
            "clip-refiner-update version=1.2.3 file=a.msi"
        );
    }
}
