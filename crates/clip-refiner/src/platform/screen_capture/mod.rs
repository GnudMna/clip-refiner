//! 画面上の矩形領域をキャプチャする

mod types;

#[cfg(windows)]
mod windows;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod xcap;

pub(crate) use types::{RgbaImage, ScreenRect};

#[cfg(windows)]
pub(crate) use windows::{capture_screen_region, virtual_screen_bounds};

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) use xcap::{capture_screen_region, virtual_screen_bounds};
