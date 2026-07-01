//! 画像からテキストを認識する (OS 標準 OCR)

mod normalize;
mod prepare;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
pub(crate) use windows::recognize_text;

#[cfg(target_os = "macos")]
pub(crate) use macos::recognize_text;

#[cfg(target_os = "linux")]
pub(crate) use linux::recognize_text;
