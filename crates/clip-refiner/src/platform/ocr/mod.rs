//! 画像からテキストを認識する (OS 標準 OCR)

mod normalize;
mod prepare;
mod recognize;

pub(crate) use recognize::recognize_text;
