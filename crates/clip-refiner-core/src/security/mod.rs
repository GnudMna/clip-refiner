//! 入力サイズ制限と機密情報の表示マスキングを提供するモジュール

mod clip_cipher;
mod fingerprint;
mod limits;
mod secret;
mod snippet;

pub use clip_cipher::{
    CIPHER_KEY_LEN, decrypt_bytes, encrypt_bytes, generate_cipher_key, zeroize_cipher_key,
};

pub use fingerprint::ContentFingerprint;
pub use limits::{is_within_clipboard_limit, is_within_parser_limit};
pub use secret::{SecretString, secret_from};
pub use snippet::format_public_snippet;
