#![windows_subsystem = "windows"]

use std::process::ExitCode;

// ======================================================================
// エントリポイント
// ======================================================================
fn main() -> ExitCode {
    clip_refiner_updater::run_updater()
}
