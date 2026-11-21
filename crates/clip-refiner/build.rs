#![allow(
    clippy::expect_used,
    reason = "ビルドスクリプトは失敗時に panic してビルドを中断するのが正規の挙動"
)]

use std::env;
use std::fmt::Write;
use std::fs;
use std::path::Path;

#[path = "src/platform/update/bridge_key_obfuscate.rs"]
mod bridge_key_obfuscate;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use zeroize::Zeroize;

use bridge_key_obfuscate::{OBFUSCATE_MASK_LEN, xor_obfuscate};

#[cfg(not(target_os = "windows"))]
compile_error!("ClipRefiner は Windows 専用です");

// ======================================================================
// 定数
// ======================================================================
/// Windows リソースの著作権表記。`LICENSE` と揃えて更新する
const LEGAL_COPYRIGHT: &str = "Copyright (c) 2026 Yuto Hirata";

// PascalCase の場合は定数として直接記述する
const APP_NAME: &str = "ClipRefiner";

/// アイコンパス
const ICON_PATH: &str = "../../assets/icon.ico";

/// ChaCha20-Poly1305 のノンス長
const NONCE_LEN: usize = 12;

/// 埋め込みキーの最大バイト長
const MAX_BRIDGE_KEY_BYTES: usize = 1024;

/// 更新署名の公開鍵 (minisign 形式 `*.pub`) を置くディレクトリ
const UPDATE_SIGNING_KEYS_DIR: &str = "../../keys/update-signing";

/// minisign 公開鍵の Base64 長 (署名方式 2 + 鍵 ID 8 + Ed25519 公開鍵 32 バイト)
const MINISIGN_PUBLIC_KEY_B64_LEN: usize = 56;

fn main() {
    println!("cargo:rerun-if-changed={ICON_PATH}");
    println!("cargo:rerun-if-changed=../../assets/icon.png");
    println!("cargo:rerun-if-changed=src/platform/update/bridge_key_obfuscate.rs");
    println!("cargo:rerun-if-env-changed=CLIP_REFINER_KEY");
    println!("cargo:rerun-if-changed={UPDATE_SIGNING_KEYS_DIR}");

    embed_bridge_key();
    embed_update_signing_keys();
    embed_windows_resources();
}

// ======================================================================
// Windows リソース
// ======================================================================
/// アイコンとバージョン情報を実行ファイルへ埋め込む
fn embed_windows_resources() {
    let mut res = winresource::WindowsResource::new();

    res.set_icon(ICON_PATH);
    res.set("ProductName", APP_NAME);
    res.set("FileDescription", APP_NAME);
    res.set("FileVersion", env!("CARGO_PKG_VERSION"));
    res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
    res.set("LegalCopyright", LEGAL_COPYRIGHT);

    if let Err(error) = res.compile() {
        panic!("Windows リソースの埋め込みに失敗: {error}");
    }
}

// ======================================================================
// 更新署名の公開鍵
// ======================================================================
/// `keys/update-signing/*.pub` の公開鍵を `OUT_DIR` へ書き出す
///
/// ファイル名順に並べる。ディレクトリや `*.pub` が無ければ空の一覧になる (署名検証は常に失敗)
fn embed_update_signing_keys() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("update_signing_keys.rs");

    let mut paths = match fs::read_dir(UPDATE_SIGNING_KEYS_DIR) {
        Ok(entries) => entries
            .map(|entry| entry.expect("更新署名鍵ディレクトリの列挙").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "pub"))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    paths.sort();

    let mut source = String::from(
        "// build.rs が生成する。keys/update-signing/*.pub の minisign 公開鍵 (Base64)\n\
         static UPDATE_SIGNING_KEYS: &[&str] = &[\n",
    );
    for path in &paths {
        let key = read_minisign_public_key(path);
        writeln!(source, "    \"{key}\",").expect("write key");
    }
    source.push_str("];\n");

    fs::write(&dest, source).unwrap_or_else(|err| {
        panic!("更新署名鍵埋め込みファイルの書き込みに失敗: {err}");
    });
}

/// minisign 形式の公開鍵ファイルから Base64 行を取り出す
///
/// 1 行目は `untrusted comment:`、2 行目が鍵本体。形式が不正ならビルドを止める
///
/// # Arguments
/// * `path` - `*.pub` ファイル
///
/// # Returns
/// * `String` - 公開鍵の Base64 文字列
fn read_minisign_public_key(path: &Path) -> String {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("更新署名鍵の読み込みに失敗: {}: {err}", path.display()));
    let mut lines = text.lines();
    let comment = lines.next().unwrap_or_default();
    let key = lines.next().unwrap_or_default().trim();
    let valid = comment.starts_with("untrusted comment:")
        && key.len() == MINISIGN_PUBLIC_KEY_B64_LEN
        && key.starts_with("RW")
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=');
    assert!(
        valid,
        "エラー: minisign 公開鍵の形式が不正です: {}",
        path.display()
    );
    key.to_string()
}

// ======================================================================
// bridge ダウンロードキー
// ======================================================================
/// `CLIP_REFINER_KEY` を暗号化して `OUT_DIR` へ書き出す
///
/// 平文は `option_env!` 経由でバイナリへ入れない
fn embed_bridge_key() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("bridge_key_blob.rs");

    match env::var("CLIP_REFINER_KEY") {
        Ok(mut key) if !key.is_empty() => {
            let source = encrypt_bridge_key(&key);
            fs::write(&dest, source).unwrap_or_else(|err| {
                panic!("bridge キー埋め込みファイルの書き込みに失敗: {err}");
            });
            key.zeroize();
        }
        Ok(_) | Err(env::VarError::NotPresent) => {
            fs::write(&dest, empty_bridge_key_source()).unwrap_or_else(|err| {
                panic!("bridge キー埋め込みファイルの書き込みに失敗: {err}");
            });
        }
        Err(env::VarError::NotUnicode(_)) => {
            panic!("エラー: 環境変数 CLIP_REFINER_KEY が Unicode ではありません");
        }
    }
}

/// キー未設定時の生成ソースを返す
///
/// # Returns
/// * `String` - `BRIDGE_KEY_PRESENT = false` の Rust ソース
fn empty_bridge_key_source() -> String {
    format!(
        "// build.rs が生成する。平文のダウンロードキーは含めない\n\
         const BRIDGE_KEY_PRESENT: bool = false;\n\
         #[allow(dead_code, reason = \"鍵未設定のビルドでは参照されない\")]\n\
         static BRIDGE_KEY_BLOB: &[u8] = &[];\n\
         #[allow(dead_code, reason = \"鍵未設定のビルドでは参照されない\")]\n\
         const BRIDGE_KEY_WRAP: [u8; {OBFUSCATE_MASK_LEN}] = [0u8; {OBFUSCATE_MASK_LEN}];\n\
         #[allow(dead_code, reason = \"鍵未設定のビルドでは参照されない\")]\n\
         const BRIDGE_KEY_MASK: [u8; {OBFUSCATE_MASK_LEN}] = [0u8; {OBFUSCATE_MASK_LEN}];\n"
    )
}

/// 平文キーを ChaCha20-Poly1305 で暗号化し、生成ソースを返す
///
/// # Arguments
/// * `plaintext` - `CLIP_REFINER_KEY` の値
///
/// # Returns
/// * `String` - 暗号化 blob を含む Rust ソース
fn encrypt_bridge_key(plaintext: &str) -> String {
    let plain_bytes = plaintext.as_bytes();
    assert!(
        plain_bytes.len() <= MAX_BRIDGE_KEY_BYTES,
        "エラー: CLIP_REFINER_KEY が長すぎます (最大 {MAX_BRIDGE_KEY_BYTES} バイト)"
    );

    let mut wrap_key = [0u8; OBFUSCATE_MASK_LEN];
    let mut mask = [0u8; OBFUSCATE_MASK_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut wrap_key).expect("wrap key 乱数");
    getrandom::fill(&mut mask).expect("mask 乱数");
    getrandom::fill(&mut nonce).expect("nonce 乱数");

    let mut blob = aead_encrypt(&wrap_key, &nonce, plain_bytes);
    nonce.zeroize();

    let mut wrap_stored = wrap_key;
    xor_obfuscate(&mut blob, &mask);
    xor_obfuscate(&mut wrap_stored, &mask);

    verify_roundtrip(plaintext.as_bytes(), &blob, &wrap_stored, &mask);

    let source = format!(
        "// build.rs が生成する。平文のダウンロードキーは含めない\n\
         const BRIDGE_KEY_PRESENT: bool = true;\n\
         static BRIDGE_KEY_BLOB: &[u8] = {};\n\
         const BRIDGE_KEY_WRAP: [u8; {OBFUSCATE_MASK_LEN}] = {};\n\
         const BRIDGE_KEY_MASK: [u8; {OBFUSCATE_MASK_LEN}] = {};\n",
        format_byte_slice(&blob),
        format_byte_array_32(&wrap_stored),
        format_byte_array_32(&mask),
    );

    wrap_key.zeroize();
    wrap_stored.zeroize();
    mask.zeroize();
    blob.zeroize();
    source
}

/// 生成直前に XOR と AEAD の往復を検証する
///
/// # Arguments
/// * `plaintext` - 元のキー
/// * `blob` - XOR 後の暗号 blob
/// * `wrap_stored` - XOR 後の wrap key
/// * `mask` - XOR マスク
fn verify_roundtrip(
    plaintext: &[u8],
    blob: &[u8],
    wrap_stored: &[u8; OBFUSCATE_MASK_LEN],
    mask: &[u8; OBFUSCATE_MASK_LEN],
) {
    let mut blob_check = blob.to_vec();
    let mut wrap_check = *wrap_stored;
    xor_obfuscate(&mut blob_check, mask);
    xor_obfuscate(&mut wrap_check, mask);
    let decrypted = aead_decrypt(&wrap_check, &blob_check)
        .expect("CLIP_REFINER_KEY の暗号化往復に失敗しました");
    assert_eq!(
        decrypted, plaintext,
        "CLIP_REFINER_KEY の暗号化往復結果が一致しません"
    );
    wrap_check.zeroize();
    blob_check.zeroize();
}

/// 平文を `nonce || ciphertext` 形式で暗号化する
///
/// # Arguments
/// * `key` - ChaCha20-Poly1305 鍵
/// * `nonce` - 12 バイトノンス
/// * `plaintext` - 暗号化対象
///
/// # Returns
/// * `Vec<u8>` - `nonce || ciphertext`
fn aead_encrypt(
    key: &[u8; OBFUSCATE_MASK_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
) -> Vec<u8> {
    let key = Key::try_from(key.as_slice()).expect("key len");
    let cipher = ChaCha20Poly1305::new(&key);
    let nonce_arr = Nonce::try_from(nonce.as_slice()).expect("nonce len");
    let ciphertext = cipher.encrypt(&nonce_arr, plaintext).expect("aead encrypt");
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(nonce);
    out.extend_from_slice(&ciphertext);
    out
}

/// `nonce || ciphertext` を復号する
///
/// # Arguments
/// * `key` - ChaCha20-Poly1305 鍵
/// * `blob` - `nonce || ciphertext`
///
/// # Returns
/// * `Option<Vec<u8>>` - 成功時は平文
fn aead_decrypt(key: &[u8; OBFUSCATE_MASK_LEN], blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() <= NONCE_LEN {
        return None;
    }
    let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
    let key = Key::try_from(key.as_slice()).ok()?;
    let cipher = ChaCha20Poly1305::new(&key);
    let nonce = Nonce::try_from(nonce).ok()?;
    cipher.decrypt(&nonce, ciphertext).ok()
}

/// スライスを Rust のバイト列リテラルへ整形する
///
/// # Arguments
/// * `bytes` - 出力するバイト
///
/// # Returns
/// * `String` - `&[0x.., ...]` 形式
fn format_byte_slice(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::from("&[]");
    }
    let mut out = String::from("&[\n");
    append_hex_bytes(&mut out, bytes, 12);
    out.push_str("\n]");
    out
}

/// 32 バイト配列を Rust リテラルへ整形する
///
/// # Arguments
/// * `bytes` - wrap key または mask
///
/// # Returns
/// * `String` - `[0x.., ...]` 形式
fn format_byte_array_32(bytes: &[u8; OBFUSCATE_MASK_LEN]) -> String {
    let mut out = String::from("[\n");
    append_hex_bytes(&mut out, bytes, 8);
    out.push_str("\n]");
    out
}

/// バイト列を `0xab,` 形式で追記する
///
/// # Arguments
/// * `out` - 出力先
/// * `bytes` - 書き出すバイト
/// * `per_line` - 1 行あたりの個数
fn append_hex_bytes(out: &mut String, bytes: &[u8], per_line: usize) {
    for (i, byte) in bytes.iter().enumerate() {
        if i % per_line == 0 {
            if i > 0 {
                out.push('\n');
            }
            out.push_str("    ");
        } else {
            out.push(' ');
        }
        write!(out, "0x{byte:02x},").expect("write hex");
    }
}
