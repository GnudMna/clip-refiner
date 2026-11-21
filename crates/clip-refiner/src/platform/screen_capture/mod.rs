//! 画面上の矩形領域をキャプチャする

mod capture;
mod types;

pub(crate) use capture::{capture_screen_region, virtual_screen_bounds};
pub(crate) use types::{RgbaImage, ScreenRect};
