//! 加工ロジック向けのネイティブ OS API (GUI 非依存)
//!
//! デスクトップ常駐の OS 連携 (通知・OCR・更新など) は `clip-refiner` の `platform/` を参照
//! `clipboard_image` は feature `clipboard`、`dpapi` は feature `config`

#[cfg(feature = "clipboard")]
pub(crate) mod clipboard_image;
#[cfg(feature = "config")]
pub(crate) mod dpapi;
