use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

use anyhow::{Context, Result};
use arboard::Clipboard;
use image;
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};

/// アプリケーションの共有状態
struct AppState {
    mode: Mutex<RefineMode>,
    paused: AtomicBool,
    interval_ms: AtomicU64,
}

impl AppState {
    fn new() -> Self {
        Self {
            mode: Mutex::new(RefineMode::UrlDecode),
            paused: AtomicBool::new(false),
            interval_ms: AtomicU64::new(1000), // デフォルト1秒
        }
    }
}

/// トレイメニューの管理
struct TrayMenu {
    _tray_icon: TrayIcon,
    quit_item: MenuItem,
    pause_item: CheckMenuItem,
    url_encode_item: CheckMenuItem,
    url_decode_item: CheckMenuItem,
    trim_item: CheckMenuItem,
    json_format_item: CheckMenuItem,
    interval_items: Vec<(CheckMenuItem, u64)>,
}

impl TrayMenu {
    fn build(state: &AppState) -> Result<Self> {
        // 加工モードメニュー
        let url_encode_item = CheckMenuItem::new("URLエンコード", true, false, None);
        let url_decode_item = CheckMenuItem::new("URLデコード", true, true, None);
        let trim_item = CheckMenuItem::new("トリム", true, false, None);
        let json_format_item = CheckMenuItem::new("JSON整形", true, false, None);
        let refine_submenu = Submenu::with_items(
            "変換モード",
            true,
            &[
                &url_encode_item,
                &url_decode_item,
                &trim_item,
                &json_format_item,
            ],
        )
        .context("変換モードメニューの作成に失敗しました")?;

        // 監視周期メニュー
        let interval_500ms = CheckMenuItem::new("0.5秒", true, false, None);
        let interval_1s = CheckMenuItem::new("1秒", true, true, None);
        let interval_2s = CheckMenuItem::new("2秒", true, false, None);
        let interval_5s = CheckMenuItem::new("5秒", true, false, None);
        let interval_items = vec![
            (interval_500ms, 500u64),
            (interval_1s, 1000u64),
            (interval_2s, 2000u64),
            (interval_5s, 5000u64),
        ];

        let mut interval_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, _) in &interval_items {
            interval_menu_items.push(item);
        }
        let interval_submenu = Submenu::with_items("監視周期", true, &interval_menu_items)
            .context("監視周期メニューの作成に失敗しました")?;

        // 一時停止・終了メニュー
        let pause_item =
            CheckMenuItem::new("一時停止", true, state.paused.load(Ordering::Relaxed), None);
        let quit_item = MenuItem::new("終了", true, None);

        // メインメニューの組み立て
        let tray_menu = Menu::new();
        tray_menu
            .append_items(&[
                &refine_submenu,
                &interval_submenu,
                &PredefinedMenuItem::separator(),
                &pause_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ])
            .context("メニューの組み立てに失敗しました")?;

        // アイコン設定
        let icon = create_icon().context("トレイアイコンの読み込みに失敗しました")?;
        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("ClipRefiner")
            .with_icon(icon)
            .build()
            .context("トレイアイコンのビルドに失敗しました")?;

        Ok(Self {
            _tray_icon,
            quit_item,
            pause_item,
            url_encode_item,
            url_decode_item,
            trim_item,
            json_format_item,
            interval_items,
        })
    }
}

/// トレイアイコンアプリケーションのメインループ
pub fn run_loop() -> Result<()> {
    let event_loop = create_event_loop();
    let state = Arc::new(AppState::new());
    let menu = TrayMenu::build(&state)?;

    // クリップボード監視スレッドの開始
    spawn_monitor_thread(Arc::clone(&state));

    let menu_channel = MenuEvent::receiver();
    let mut clipboard = init_clipboard()?;

    // イベントループの実行
    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(event) = menu_channel.try_recv() {
            handle_menu_event(event, &menu, &state, &mut clipboard, control_flow);
        }
    })
}

/// イベントループの作成
fn create_event_loop() -> EventLoop<()> {
    #[cfg(windows)]
    return EventLoopBuilder::new().with_any_thread(true).build();

    #[cfg(not(windows))]
    return EventLoopBuilder::new().build();
}

/// クリップボード監視スレッドの開始
fn spawn_monitor_thread(state: Arc<AppState>) {
    thread::spawn(move || {
        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                notification::error::show_anyhow_error("監視スレッドエラー", &e);
                return;
            }
        };

        let mut last_text = clipboard.get_text().unwrap_or_default();

        loop {
            let interval = state.interval_ms.load(Ordering::Relaxed);
            thread::sleep(Duration::from_millis(interval));

            if state.paused.load(Ordering::Relaxed) {
                continue;
            }

            if let Ok(text) = clipboard.get_text() {
                if !text.is_empty() && text != last_text {
                    let current_mode = *state.mode.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(processed) = process_clipboard(&mut clipboard, current_mode) {
                        last_text = processed;
                        continue;
                    }
                }
                last_text = text;
            }
        }
    });
}

/// メニューイベントのハンドリング
fn handle_menu_event(
    event: MenuEvent,
    menu: &TrayMenu,
    state: &AppState,
    clipboard: &mut Clipboard,
    control_flow: &mut ControlFlow,
) {
    if event.id == menu.quit_item.id() {
        *control_flow = ControlFlow::Exit;
    } else if event.id == menu.pause_item.id() {
        state
            .paused
            .store(menu.pause_item.is_checked(), Ordering::Relaxed);
    } else if event.id == menu.url_encode_item.id() {
        update_refine(state, menu, clipboard, RefineMode::UrlEncode);
    } else if event.id == menu.url_decode_item.id() {
        update_refine(state, menu, clipboard, RefineMode::UrlDecode);
    } else if event.id == menu.trim_item.id() {
        update_refine(state, menu, clipboard, RefineMode::Trim);
    } else if event.id == menu.json_format_item.id() {
        update_refine(state, menu, clipboard, RefineMode::JsonFormat);
    } else {
        for (item, ms) in &menu.interval_items {
            if event.id == item.id() {
                state.interval_ms.store(*ms, Ordering::Relaxed);
                for (it, _) in &menu.interval_items {
                    it.set_checked(false);
                }
                item.set_checked(true);
                break;
            }
        }
    }
}

/// 加工モードの更新
fn update_refine(state: &AppState, menu: &TrayMenu, clipboard: &mut Clipboard, mode: RefineMode) {
    *state.mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;

    menu.url_encode_item
        .set_checked(mode == RefineMode::UrlEncode);
    menu.url_decode_item
        .set_checked(mode == RefineMode::UrlDecode);
    menu.trim_item.set_checked(mode == RefineMode::Trim);
    menu.json_format_item
        .set_checked(mode == RefineMode::JsonFormat);

    process_clipboard(clipboard, mode);
}

/// クリップボードの初期化
fn init_clipboard() -> Result<Clipboard> {
    Clipboard::new().context("クリップボードのアクセスに失敗しました")
}

/// トレイアイコンの作成
fn create_icon() -> Result<Icon> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let img =
        image::load_from_memory(icon_bytes).context("アイコン画像のデコードに失敗しました")?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let rgba_raw = rgba.into_raw();

    Icon::from_rgba(rgba_raw, width, height).context("アイコンデータの作成に失敗しました")
}
