use super::types::{RgbaImage, ScreenRect};

use anyhow::{Context, Result, bail};

// ======================================================================
// パブリック関数
// ======================================================================
/// 接続済みモニタ全体を覆う境界 (物理ピクセル) を返す
pub(crate) fn virtual_screen_bounds() -> (i32, i32, u32, u32) {
    match xcap::Monitor::all() {
        Ok(monitors) if !monitors.is_empty() => union_monitor_bounds(&monitors),
        _ => (0, 0, 0, 0),
    }
}

/// 指定矩形の画面領域を RGBA 画像として取得する
pub(crate) fn capture_screen_region(rect: ScreenRect) -> Result<RgbaImage> {
    if rect.width == 0 || rect.height == 0 {
        bail!("キャプチャ領域が空");
    }

    let monitors = xcap::Monitor::all().context("モニタ一覧の取得に失敗")?;
    if monitors.is_empty() {
        bail!("利用可能なモニタがありません");
    }

    let (monitor_index, local_rect) = find_monitor_for_rect(&monitors, rect)?;
    let monitor = &monitors[monitor_index];
    let image = monitor
        .capture_region(
            local_rect.x,
            local_rect.y,
            local_rect.width,
            local_rect.height,
        )
        .with_context(|| {
            format!(
                "領域 ({}, {}, {}x{}) のキャプチャに失敗",
                local_rect.x, local_rect.y, local_rect.width, local_rect.height
            )
        })?;

    let width = u32::try_from(image.width()).context("キャプチャ幅の変換に失敗")?;
    let height = u32::try_from(image.height()).context("キャプチャ高さの変換に失敗")?;
    let rgba = image.into_raw();

    Ok(RgbaImage {
        width,
        height,
        rgba,
    })
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 全モニタを包含する矩形を返す
fn union_monitor_bounds(monitors: &[xcap::Monitor]) -> (i32, i32, u32, u32) {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for monitor in monitors {
        if let (Ok(x), Ok(y), Ok(width), Ok(height)) =
            (monitor.x(), monitor.y(), monitor.width(), monitor.height())
        {
            let right = x.saturating_add(width.cast_signed());
            let bottom = y.saturating_add(height.cast_signed());
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(right);
            max_y = max_y.max(bottom);
        }
    }

    if min_x == i32::MAX {
        return (0, 0, 0, 0);
    }

    (
        min_x,
        min_y,
        u32::try_from(max_x.saturating_sub(min_x)).unwrap_or(0),
        u32::try_from(max_y.saturating_sub(min_y)).unwrap_or(0),
    )
}

/// 矩形の中心点を含むモニタと、モニタローカル座標へ変換した矩形を返す
fn find_monitor_for_rect(
    monitors: &[xcap::Monitor],
    rect: ScreenRect,
) -> Result<(usize, ScreenRect)> {
    let center_x = rect.x.saturating_add(rect.width.cast_signed() / 2);
    let center_y = rect.y.saturating_add(rect.height.cast_signed() / 2);

    for (index, monitor) in monitors.iter().enumerate() {
        let (Ok(mx), Ok(my), Ok(mw), Ok(mh)) =
            (monitor.x(), monitor.y(), monitor.width(), monitor.height())
        else {
            continue;
        };

        let right = mx.saturating_add(mw.cast_signed());
        let bottom = my.saturating_add(mh.cast_signed());
        if center_x >= mx && center_x < right && center_y >= my && center_y < bottom {
            let local = ScreenRect {
                x: rect.x.saturating_sub(mx),
                y: rect.y.saturating_sub(my),
                width: rect.width,
                height: rect.height,
            };
            return Ok((index, local));
        }
    }

    bail!("選択範囲を含むモニタが見つかりません");
}
