use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use crate::coder::{decoder, encoder};
use crate::notification;

use arboard::Clipboard;
use image;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[derive(PartialEq, Debug, Clone, Copy)]
enum CodecMode {
    Encode,
    Decode,
}

/// トレイアイコンアプリケーションのメインループ
pub fn run_loop() -> Result<(), Box<dyn std::error::Error>> {
    // イベントループの作成（Windows専用）
    #[cfg(windows)]
    let event_loop = EventLoopBuilder::new().with_any_thread(true).build();

    // イベントループの作成（Windows以外）
    // Linuxの場合は以下のライブラリが必要な可能性があり
    // * libgtk-3-dev
    // * libappindicator3-dev
    #[cfg(not(windows))]
    let event_loop = EventLoopBuilder::new().build();

    // コーデックメニュー
    let encode_item = CheckMenuItem::new("エンコード", true, false, None);
    let decode_item = CheckMenuItem::new("デコード", true, true, None);
    let codec_menu_item =
        tray_icon::menu::Submenu::with_items("コーデック", true, &[&encode_item, &decode_item])?;

    // 監視周期メニュー
    let interval_500ms = CheckMenuItem::new("0.5秒", true, false, None);
    let interval_1s = CheckMenuItem::new("1秒", true, true, None);
    let interval_2s = CheckMenuItem::new("2秒", true, false, None);
    let interval_5s = CheckMenuItem::new("5秒", true, false, None);
    let interval_menu_item = tray_icon::menu::Submenu::with_items(
        "監視周期",
        true,
        &[&interval_500ms, &interval_1s, &interval_2s, &interval_5s],
    )?;

    // 一時停止メニュー
    let pause_item = CheckMenuItem::new("一時停止", true, false, None);

    // 終了メニュー
    let quit_item = MenuItem::new("終了", true, None);

    // メニューのセットアップ
    let tray_menu = Menu::new();
    tray_menu.append_items(&[
        &codec_menu_item,
        &interval_menu_item,
        &PredefinedMenuItem::separator(),
        &pause_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])?;

    // アイコン設定
    let icon = match create_icon() {
        Ok(i) => i,
        Err(e) => {
            let msg = format!("アイコンの読み込みに失敗しました: {}", e);
            eprintln!("{}", msg);
            notification::error::show_error_notification("起動エラー", &msg);
            return Err(e);
        }
    };
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("ClipCoder")
        .with_icon(icon)
        .build()?;

    // 共有状態
    let mode = Arc::new(std::sync::Mutex::new(CodecMode::Decode));
    let paused = Arc::new(AtomicBool::new(false));
    let interval_ms = Arc::new(std::sync::atomic::AtomicU64::new(1000)); // デフォルト1秒

    // クリップボード監視スレッド用の共有状態
    let thread_mode = mode.clone();
    let thread_paused = paused.clone();
    let thread_interval = interval_ms.clone();

    // クリップボード監視スレッド
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                let msg = format!("クリップボードの初期化に失敗しました: {}", e);
                eprintln!("{}", msg);
                notification::error::show_error_notification("初期化エラー", &msg);
                return;
            }
        };

        let mut last_text = String::new();
        // 即座に処理されないように、現在の内容でlast_textを初期化
        if let Ok(text) = clipboard.get_text() {
            last_text = text;
        }

        loop {
            let interval = thread_interval.load(Ordering::Relaxed);
            thread::sleep(Duration::from_millis(interval));

            if thread_paused.load(Ordering::Relaxed) {
                continue;
            }

            match clipboard.get_text() {
                Ok(text) => {
                    if text.is_empty() {
                        continue; // 空のテキストは無視
                    } else if text != last_text {
                        let current_mode = *thread_mode.lock().unwrap();
                        if let Some(processed) = process_clipboard(current_mode) {
                            // processed はすでに clipboard に書き込まれている
                            last_text = processed;
                            continue;
                        }
                    }
                    last_text = text;
                }
                Err(e) => {
                    // クリップボードへのアクセス自体が失敗した場合は、一時的なエラーの可能性もあるが
                    // 回復不能なエラー（初期化ミスなど）はすでに上で処理している。
                    // ここではデバッグ用にログを出す程度に止める（スレッドは継続）
                    eprintln!("Error reading clipboard: {}", e);
                }
            }
        }
    });

    let menu_channel = MenuEvent::receiver();

    // イベントループ
    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_item.id() {
                // 終了メニュー
                *control_flow = ControlFlow::Exit;
            } else if event.id == pause_item.id() {
                // 一時停止メニュー
                let new_state = pause_item.is_checked();
                paused.store(new_state, Ordering::Relaxed);
            } else {
                // メニュー項目の配列
                let codecs = [
                    (&encode_item, CodecMode::Encode),
                    (&decode_item, CodecMode::Decode),
                ];
                let intervals = [
                    (&interval_500ms, 500u64),
                    (&interval_1s, 1000u64),
                    (&interval_2s, 2000u64),
                    (&interval_5s, 5000u64),
                ];

                // コーデックメニュー
                let mut codec_handled = false;
                for (item, app_mode) in &codecs {
                    if event.id == item.id() {
                        *mode.lock().unwrap() = *app_mode;
                        // すべてのチェックを外してから選択されたものだけチェック
                        for (check_item, _) in &codecs {
                            check_item.set_checked(false);
                        }
                        item.set_checked(true);

                        // クリップボードの内容を変換
                        process_clipboard(*app_mode);

                        codec_handled = true;
                        break;
                    }
                }

                // 監視周期メニュー
                if !codec_handled {
                    for (item, ms) in &intervals {
                        if event.id == item.id() {
                            interval_ms.store(*ms, Ordering::Relaxed);
                            // すべてのチェックを外してから選択されたものだけチェック
                            for (check_item, _) in &intervals {
                                check_item.set_checked(false);
                            }
                            item.set_checked(true);
                            break;
                        }
                    }
                }
            }
        }
    })
}

/// クリップボードの内容を変換
fn process_clipboard(mode: CodecMode) -> Option<String> {
    let mut clipboard = Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;

    if text.is_empty() {
        return None;
    }

    let processed = match mode {
        CodecMode::Encode => encoder::percent_encode_text(&text),
        CodecMode::Decode => decoder::percent_decode_text(&text).unwrap_or(text.clone()),
    };

    if processed != text {
        let _ = clipboard.set_text(processed.clone());
        return Some(processed);
    }

    None
}

/// トレイアイコンの作成
fn create_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let img = image::load_from_memory(icon_bytes)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let rgba_raw = rgba.into_raw();

    Icon::from_rgba(rgba_raw, width, height).map_err(|e| e.into())
}
