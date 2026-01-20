use clap::ValueEnum;

pub mod decoder;
pub mod encoder;

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum CodecMode {
    Encode,
    Decode,
}
