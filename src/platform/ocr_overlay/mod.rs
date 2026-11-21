//! 画面範囲選択用のネイティブオーバーレイ

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::OverlayWindow;
