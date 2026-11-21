//! 入力サイズ制限と機密情報の表示マスキングを提供するモジュール
//!
//! マスキングは通知・トレイメニュー・プレビューなど UI 表示の置換のみ
//! クリップボード本体や履歴復元・登録クリップコピー時の本文は書き換えない
//! AEAD・指紋・`SecretString` は feature `config` が必要

#[cfg(feature = "config")]
mod clip_cipher;
#[cfg(feature = "config")]
mod fingerprint;
mod limits;
#[cfg(feature = "config")]
mod secret;
mod snippet;

#[cfg(feature = "config")]
pub use clip_cipher::{
    CIPHER_KEY_LEN, decrypt_bytes, encrypt_bytes, generate_cipher_key, zeroize_cipher_key,
};
#[cfg(feature = "config")]
pub use fingerprint::ContentFingerprint;
pub use limits::{is_within_clipboard_limit, is_within_parser_limit};
#[cfg(feature = "config")]
pub use secret::{SecretString, secret_from};
pub use snippet::{format_public_snippet, looks_sensitive};
