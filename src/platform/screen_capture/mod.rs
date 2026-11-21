//! 画面上の矩形領域をキャプチャする

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::{RgbaImage, ScreenRect, capture_screen_region, virtual_screen_bounds};

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    /// 画面左上からの X 座標 (物理ピクセル)
    pub x: i32,
    /// 画面左上からの Y 座標 (物理ピクセル)
    pub y: i32,
    /// 幅 (物理ピクセル)
    pub width: u32,
    /// 高さ (物理ピクセル)
    pub height: u32,
}

#[cfg(not(windows))]
#[derive(Debug, Clone)]
pub(crate) struct RgbaImage {
    /// 画像幅 (ピクセル)
    pub width: u32,
    /// 画像高さ (ピクセル)
    pub height: u32,
    /// RGBA ピクセル列 (左上から行優先)
    pub rgba: Vec<u8>,
}

#[cfg(not(windows))]
pub(crate) fn virtual_screen_bounds() -> (i32, i32, u32, u32) {
    (0, 0, 0, 0)
}

#[cfg(not(windows))]
pub(crate) fn capture_screen_region(_rect: ScreenRect) -> anyhow::Result<RgbaImage> {
    anyhow::bail!("このプラットフォームでは画面キャプチャに未対応")
}
