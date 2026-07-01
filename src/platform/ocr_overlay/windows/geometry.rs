use super::types::SelectionRect;

use crate::platform::screen_capture::ScreenRect;

use windows_sys::Win32::Foundation::LPARAM;

// ======================================================================
// 座標変換
// ======================================================================
/// UTF-8 文字列を null 終端付き UTF-16 へ変換する
pub(crate) fn str_to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `LPARAM` からクライアント X 座標を取り出す
pub(crate) fn lparam_x(lparam: LPARAM) -> i32 {
    i32::from(u16::try_from(lparam & 0xFFFF).unwrap_or(0))
}

/// `LPARAM` からクライアント Y 座標を取り出す
pub(crate) fn lparam_y(lparam: LPARAM) -> i32 {
    let high = u32::try_from(lparam).unwrap_or(0) >> 16;
    i32::from(u16::try_from(high).unwrap_or(0))
}

/// 2点から正規化した選択矩形を作る
pub(crate) fn normalize_rect(start: (i32, i32), end: (i32, i32)) -> SelectionRect {
    let x1 = start.0.min(end.0);
    let y1 = start.1.min(end.1);
    let x2 = start.0.max(end.0);
    let y2 = start.1.max(end.1);
    SelectionRect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1).max(1),
        height: y2.saturating_sub(y1).max(1),
    }
}

/// クライアント座標の矩形を仮想デスクトップ座標へ変換する
pub(crate) fn map_client_rect_to_screen(
    virt_x: i32,
    virt_y: i32,
    rect: SelectionRect,
) -> ScreenRect {
    ScreenRect {
        x: virt_x.saturating_add(rect.x),
        y: virt_y.saturating_add(rect.y),
        width: u32::try_from(rect.width.max(1)).unwrap_or(1),
        height: u32::try_from(rect.height.max(1)).unwrap_or(1),
    }
}

/// 矩形を外側へ拡張する
pub(crate) fn expand_rect(rect: SelectionRect, margin: i32) -> SelectionRect {
    SelectionRect {
        x: rect.x.saturating_sub(margin),
        y: rect.y.saturating_sub(margin),
        width: rect.width.saturating_add(margin.saturating_mul(2)),
        height: rect.height.saturating_add(margin.saturating_mul(2)),
    }
}

/// 矩形をフレーム内に収める
pub(crate) fn clamp_rect(
    rect: SelectionRect,
    frame_width: u32,
    frame_height: u32,
) -> Option<SelectionRect> {
    let width_i = i32::try_from(frame_width).ok()?;
    let height_i = i32::try_from(frame_height).ok()?;
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    let x1 = rect.x.clamp(0, width_i.saturating_sub(1));
    let y1 = rect.y.clamp(0, height_i.saturating_sub(1));
    let x2 = rect
        .x
        .saturating_add(rect.width)
        .clamp(0, width_i)
        .max(x1.saturating_add(1));
    let y2 = rect
        .y
        .saturating_add(rect.height)
        .clamp(0, height_i)
        .max(y1.saturating_add(1));

    Some(SelectionRect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1),
        height: y2.saturating_sub(y1),
    })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 2点から正規化矩形を作る
    #[test]
    fn normalize_rect_orders_corners() {
        let rect = normalize_rect((30, 40), (10, 20));
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 20);
        assert_eq!(rect.height, 20);
    }

    /// クライアント座標から仮想デスクトップ座標への変換を検証する
    #[test]
    fn map_client_rect_to_screen_adds_virtual_origin() {
        let rect = map_client_rect_to_screen(
            -100,
            50,
            SelectionRect {
                x: 100,
                y: 200,
                width: 300,
                height: 400,
            },
        );
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 250);
        assert_eq!(rect.width, 300);
        assert_eq!(rect.height, 400);
    }
}
