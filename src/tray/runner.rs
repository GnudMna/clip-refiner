use super::app::App;
use super::state::AppEvent;

use anyhow::Result;
use tao::event_loop::{EventLoop, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;

// ======================================================================
// メインループ
// ======================================================================
/// アプリケーションのメインイベントループを開始する
///
/// イベントループを初期化し、`App` 構造体を生成して実行します。
/// この関数はアプリケーションが終了するまでブロックされます。
///
/// # Returns
/// * `Result<()>` - アプリケーションが正常に終了した場合は `Ok(())`、起動や実行中にエラーが発生した場合は `Err` を返します。
pub fn run_loop() -> Result<()> {
    let event_loop = create_event_loop();
    let proxy = event_loop.create_proxy();

    let mut app = App::new(&event_loop, proxy)?;

    event_loop.run(move |event, _, control_flow| {
        app.handle_event(event, control_flow);
    })
}

// ======================================================================
// イベントループ作成
// ======================================================================
/// 実行プラットフォームに適した設定でイベントループを作成する
///
/// # Returns
/// * `EventLoop<AppEvent>` - 生成された `tao` のイベントループ
fn create_event_loop() -> EventLoop<AppEvent> {
    #[cfg(windows)]
    return EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    #[cfg(not(windows))]
    return EventLoopBuilder::<AppEvent>::with_user_event().build();
}
