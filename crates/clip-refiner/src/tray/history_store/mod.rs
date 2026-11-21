//! セッション限定の暗号化クリップボード履歴ストア
//!
//! トレイメニューの履歴 UI は [`crate::tray::features::history`] が担う

mod store;

pub use store::EncryptedHistoryStore;

/// 連続記録用の暗号化ストア (`EncryptedHistoryStore` と同一実装)
pub type EncryptedRecordStore = EncryptedHistoryStore;
