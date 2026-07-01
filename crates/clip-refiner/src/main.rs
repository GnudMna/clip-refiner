#![cfg_attr(windows, windows_subsystem = "windows")] // Windowsでコンソールを出さないための設定

// ======================================================================
// エントリポイント
// ======================================================================
fn main() -> anyhow::Result<()> {
    #[cfg(feature = "app")]
    return clip_refiner::run();

    #[cfg(not(feature = "app"))]
    anyhow::bail!("ClipRefiner バイナリには feature `app` が必要です")
}
