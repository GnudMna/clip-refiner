pub mod decoder;
pub mod encoder;

use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum CodecMode {
    Encode,
    Decode,
}
