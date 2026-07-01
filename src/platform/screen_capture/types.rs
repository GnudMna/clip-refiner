// ======================================================================
// 画面矩形・画像バッファ
// ======================================================================
/// 画面上の矩形領域 (物理ピクセル座標)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    /// 画面左上からの X 座標
    pub x: i32,
    /// 画面左上からの Y 座標
    pub y: i32,
    /// 幅
    pub width: u32,
    /// 高さ
    pub height: u32,
}

/// キャプチャした RGBA 画像
#[derive(Debug, Clone)]
pub(crate) struct RgbaImage {
    /// 画像幅 (ピクセル)
    pub width: u32,
    /// 画像高さ (ピクセル)
    pub height: u32,
    /// RGBA ピクセル列 (左上から行優先)
    pub rgba: Vec<u8>,
}
