//! 加工ロジック向けのネイティブ OS API (GUI 非依存)
//!
//! デスクトップ常駐の OS 連携 (通知・OCR・更新など) は `clip-refiner` の `platform/` を参照
//! `dpapi` は feature `config`、それ以外は feature `clipboard`

#[cfg(feature = "clipboard")]
pub(crate) mod clipboard_exclusion;
#[cfg(feature = "clipboard")]
pub(crate) mod clipboard_file;
#[cfg(feature = "clipboard")]
pub(crate) mod clipboard_image;
#[cfg(feature = "config")]
pub(crate) mod dpapi;
#[cfg(feature = "clipboard")]
pub(crate) mod stale_image_files;
