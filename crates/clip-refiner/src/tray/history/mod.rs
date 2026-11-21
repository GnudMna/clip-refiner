//! セッション限定の暗号化クリップボード履歴

mod store;

pub use store::EncryptedHistoryStore;

/// 連続記録用の暗号化ストア (`EncryptedHistoryStore` と同一実装)
pub type EncryptedRecordStore = EncryptedHistoryStore;
