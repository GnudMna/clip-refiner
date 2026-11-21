use std::io::Cursor;

use anyhow::{Context, Result};
use tao::window::Icon as WindowIcon;
use tray_icon::Icon as TrayIcon;

// ======================================================================
// アイコン作成
// ======================================================================
/// 埋め込み `assets/icon.png` を RGBA8 配列へデコードする
fn decode_app_icon_png() -> Result<(Vec<u8>, u32, u32)> {
    let icon_bytes = include_bytes!("../../../../../assets/icon.png");
    let decoder = png::Decoder::new(Cursor::new(icon_bytes.as_slice()));
    let mut reader = decoder
        .read_info()
        .context("アイコンPNGのデコードに失敗しました")?;
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buf)
        .context("アイコンPNGフレームの読み取りに失敗しました")?;
    let bytes = &buf[..info.buffer_size()];

    // カラータイプに応じて RGBA8 に変換する
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => anyhow::bail!("サポート外PNGカラータイプ: Indexed"),
    };

    Ok((rgba, info.width, info.height))
}

/// 埋め込まれたアセットからトレイ用のアイコンを作成する
///
/// # Returns
/// * `Result<TrayIcon>` - 作成されたアイコン。失敗した場合はエラーを返す
pub fn create_icon() -> Result<TrayIcon> {
    let (rgba, width, height) = decode_app_icon_png()?;
    TrayIcon::from_rgba(rgba, width, height).context("トレイアイコンデータの作成に失敗しました")
}

/// 埋め込まれたアセットからウィンドウ用のアイコンを作成する
///
/// # Returns
/// * `Result<WindowIcon>` - 作成されたアイコン。失敗した場合はエラーを返す
pub fn create_window_icon() -> Result<WindowIcon> {
    let (rgba, width, height) = decode_app_icon_png()?;
    WindowIcon::from_rgba(rgba, width, height)
        .context("ウィンドウアイコンデータの作成に失敗しました")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 埋め込み PNG からトレイアイコンを生成できるようにすること
    #[test]
    fn create_icon_from_embedded_png() {
        let _icon = create_icon().expect("トレイアイコンの作成に失敗");
    }

    /// 埋め込み PNG からウィンドウアイコンを生成できるようにすること
    #[test]
    fn create_window_icon_from_embedded_png() {
        let _icon = create_window_icon().expect("ウィンドウアイコンの作成に失敗");
    }
}
