//! 画像からテキストを認識する (OS 標準 OCR)

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::recognize_text;

#[cfg(not(windows))]
pub(crate) fn recognize_text(_image: &super::screen_capture::RgbaImage) -> anyhow::Result<String> {
    anyhow::bail!("このプラットフォームでは OCR に未対応")
}
