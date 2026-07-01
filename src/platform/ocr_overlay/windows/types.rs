use crate::platform::screen_capture::ScreenRect;

// ======================================================================
// 型
// ======================================================================
/// オーバーレイ上の選択矩形 (クライアント座標)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionRect {
    /// 左上 X
    pub x: i32,
    /// 左上 Y
    pub y: i32,
    /// 幅
    pub width: i32,
    /// 高さ
    pub height: i32,
}

/// ドラッグ選択の状態
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DragSelection {
    /// ドラッグ開始点 (クライアント座標)
    pub start: Option<(i32, i32)>,
    /// 現在のカーソル位置 (クライアント座標)
    pub cursor: Option<(i32, i32)>,
}

/// 選択確定時に呼ばれるコールバック
pub(crate) type OverlayCompleteFn = Box<dyn Fn(ScreenRect)>;

// ======================================================================
// 定数
// ======================================================================
/// 画面全体の暗転アルファ (約 35%)
pub(crate) const DIM_ALPHA: u8 = 89;

/// 選択枠の線幅
pub(crate) const BORDER_WIDTH: i32 = 2;

/// 選択確定に必要な最小辺長
pub(crate) const MIN_SELECTION_SIZE: i32 = 5;

/// ドラッグ中の再描画間隔
pub(crate) const REDRAW_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

/// ウィンドウクラス名
pub(crate) const OVERLAY_CLASS_NAME: &str = "ClipRefinerOcrOverlay";

/// `RegisterClassW` が返す「クラスは既に存在する」エラー
pub(crate) const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;

/// 選択枠の BGRA 色
pub(crate) const BORDER_PIXEL: [u8; 4] = [0xFF, 0xA8, 0x5A, 0xFF];
