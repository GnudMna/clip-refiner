use anyhow::Result;
use tao::event_loop::{EventLoop, EventLoopBuilder};
use tao::platform::windows::EventLoopBuilderExtWindows;

use super::app::App;
use super::e2e::E2eWindow;
use super::state::{AppEvent, SelectorEvent, SettingsEvent};

// ======================================================================
// メインループ
// ======================================================================
/// アプリケーションのメインイベントループを開始する
///
/// イベントループを初期化し、`App` 構造体を生成して実行する
/// この関数はアプリケーションが終了するまでブロックされる
///
/// # Arguments
/// * `e2e_window` - E2E 時に自動表示するウィンドウ (通常起動では `None`)
///
/// # Returns
/// * `Result<()>` - アプリケーションが正常に終了した場合は `Ok(())`、起動や実行中にエラーが発生した場合は `Err` を返す
pub fn run_loop(e2e_window: Option<E2eWindow>) -> Result<()> {
    let event_loop = create_event_loop();
    let proxy = event_loop.create_proxy();

    let mut app = App::new(&proxy)?;
    if e2e_window.is_none() {
        clip_refiner_updater::notify_startup_complete();
    }

    if let Some(target) = e2e_window {
        let event = match target {
            E2eWindow::Settings => SettingsEvent::Show.into(),
            E2eWindow::Quick => SelectorEvent::ShowQuick.into(),
            E2eWindow::Clip => SelectorEvent::ShowClip.into(),
        };
        let _ = proxy.send_event(event);
    }

    event_loop.run(move |event, event_loop, control_flow| {
        app.handle_event(event, event_loop, control_flow);
    })
}

// ======================================================================
// イベントループ作成
// ======================================================================
/// Windows 向けの設定でイベントループを作成する
///
/// # Returns
/// * `EventLoop<AppEvent>` - 生成された `tao` のイベントループ
fn create_event_loop() -> EventLoop<AppEvent> {
    EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// イベントループを生成できるようにすること
    #[test]
    fn create_event_loop_builds() {
        let _ = create_event_loop();
    }
}
