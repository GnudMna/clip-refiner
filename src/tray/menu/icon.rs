use anyhow::{Context, Result};
use tray_icon::Icon;

// ======================================================================
// アイコン作成
// ======================================================================
/// 埋め込まれたアセットからトレイ用のアイコンを作成する
///
/// # Returns
/// * `Result<Icon>` - 作成されたアイコン。失敗した場合はエラーを返す。
pub fn create_icon() -> Result<Icon> {
    use std::io::Cursor;

    let icon_bytes = include_bytes!("../../../assets/icon.png");
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

    Icon::from_rgba(rgba, info.width, info.height).context("アイコンデータの作成に失敗しました")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 埋め込み PNG からトレイアイコンを生成できること
    #[test]
    fn create_icon_from_embedded_png() {
        let _icon = create_icon().expect("トレイアイコンの作成に失敗");
    }
}
