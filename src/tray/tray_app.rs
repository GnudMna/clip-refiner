use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

use crate::config::{AppConfig, MonitorMode};
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
    monitor_mode: Mutex<MonitorMode>,
    monitor_generation: AtomicU64,
    interval_ms: AtomicU64,
    last_processed_text: Mutex<String>,
}

impl AppState {
    fn new() -> Self {
        let config = AppConfig::load();
        Self {
            mode: Mutex::new(config.mode),
            paused: AtomicBool::new(false),
            monitor_mode: Mutex::new(config.monitor_mode),
            monitor_generation: AtomicU64::new(0),
            interval_ms: AtomicU64::new(config.interval_ms),
            last_processed_text: Mutex::new(String::new()),
        }
    }

    /// 設定を保存する
    fn save_config(&self) {
        let config = AppConfig {
            mode: self.get_mode(),
            interval_ms: self.interval_ms.load(Ordering::Relaxed),
            monitor_mode: self.get_monitor_mode(),
        };
        if let Err(e) = config.save() {
            eprintln!("設定の保存に失敗: {}", e);
        }
    }

    fn get_mode(&self) -> RefineMode {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_mode(&self, mode: RefineMode) {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;
    }

    fn get_monitor_mode(&self) -> MonitorMode {
        *self.monitor_mode.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_monitor_mode(&self, mode: MonitorMode) {
        *self.monitor_mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;
    }

    fn get_last_processed_text(&self) -> String {
        self.last_processed_text
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_last_processed_text(&self, text: String) {
        *self
            .last_processed_text
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = text;
    }
}

/// トレイメニューの管理
struct TrayMenu {
    _tray_icon: TrayIcon,
    quit_item: MenuItem,
    pause_item: CheckMenuItem,
    mode_items: Vec<(CheckMenuItem, RefineMode)>,
    json_format_items: Vec<(CheckMenuItem, RefineMode)>,
    json_to_yaml_items: Vec<(CheckMenuItem, RefineMode)>,
    yaml_to_json_items: Vec<(CheckMenuItem, RefineMode)>,
    monitor_mode_items: Vec<(CheckMenuItem, MonitorMode)>,
    interval_submenu: Submenu,
    interval_items: Vec<(CheckMenuItem, u64)>,
}

impl TrayMenu {
    fn build(state: &AppState) -> Result<Self> {
        let current_mode = state.get_mode();
        let current_interval = state.interval_ms.load(Ordering::Relaxed);

        // JSON整形 / JSON→YAML / YAML→JSON のサブメニュー用アイテム
        let mut json_format_items = Vec::new();
        let mut json_to_yaml_items = Vec::new();
        let mut yaml_to_json_items = Vec::new();

        // 変換モードアイテム
        let mut mode_items = Vec::new();

        for &mode in RefineMode::variants() {
            let item = CheckMenuItem::new(mode.label(), true, mode == current_mode, None);
            match mode.category() {
                crate::refiner::RefineCategory::Normal => mode_items.push((item, mode)),
                crate::refiner::RefineCategory::JsonFormat => json_format_items.push((item, mode)),
                crate::refiner::RefineCategory::JsonToYaml => json_to_yaml_items.push((item, mode)),
                crate::refiner::RefineCategory::YamlToJson => yaml_to_json_items.push((item, mode)),
            }
        }

        // サブメニューの作成
        let json_format_submenu = Submenu::with_items(
            "JSON整形",
            true,
            &json_format_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;
        let json_to_yaml_submenu = Submenu::with_items(
            "JSON→YAML",
            true,
            &json_to_yaml_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;
        let yaml_to_json_submenu = Submenu::with_items(
            "YAML→JSON",
            true,
            &yaml_to_json_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;

        // メインの変換モードメニュー組み立て
        let mut mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, mode) in &mode_items {
            mode_menu_items.push(item);
            // 特定の項目の後にサブメニューを配置
            if *mode == RefineMode::TrimLines {
                mode_menu_items.push(&json_format_submenu);
                mode_menu_items.push(&json_to_yaml_submenu);
                mode_menu_items.push(&yaml_to_json_submenu);
            }
        }

        let refine_submenu = Submenu::with_items("変換モード", true, &mode_menu_items)
            .context("変換モードメニューの作成に失敗しました")?;

        // 監視モードメニュー
        let current_monitor_mode = *state.monitor_mode.lock().unwrap_or_else(|e| e.into_inner());
        let polling_item = CheckMenuItem::new(
            "ポーリング",
            true,
            current_monitor_mode == MonitorMode::Polling,
            None,
        );

        #[cfg(windows)]
        let event_item = CheckMenuItem::new(
            "イベント",
            true,
            current_monitor_mode == MonitorMode::Event,
            None,
        );

        #[cfg(windows)]
        let monitor_mode_items = vec![
            (polling_item, MonitorMode::Polling),
            (event_item, MonitorMode::Event),
        ];

        #[cfg(not(windows))]
        let monitor_mode_items = vec![(polling_item, MonitorMode::Polling)];

        let mut monitor_mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, _) in &monitor_mode_items {
            monitor_mode_menu_items.push(item);
        }
        let monitor_mode_submenu = Submenu::with_items("監視方式", true, &monitor_mode_menu_items)
            .context("監視方式メニューの作成に失敗しました")?;

        // 監視周期メニュー
        let interval_500ms = CheckMenuItem::new("0.5秒", true, current_interval == 500, None);
        let interval_1s = CheckMenuItem::new("1秒", true, current_interval == 1000, None);
        let interval_2s = CheckMenuItem::new("2秒", true, current_interval == 2000, None);
        let interval_5s = CheckMenuItem::new("5秒", true, current_interval == 5000, None);
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

        // イベントモードの場合は監視周期を無効化
        #[cfg(windows)]
        if current_monitor_mode == MonitorMode::Event {
            interval_submenu.set_enabled(false);
        }

        // 一時停止・終了メニュー
        let pause_item =
            CheckMenuItem::new("一時停止", true, state.paused.load(Ordering::Relaxed), None);
        let quit_item = MenuItem::new("終了", true, None);

        // メインメニューの組み立て
        let tray_menu = Menu::new();
        tray_menu
            .append_items(&[
                &refine_submenu,
                &monitor_mode_submenu,
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
            mode_items,
            json_format_items,
            json_to_yaml_items,
            yaml_to_json_items,
            monitor_mode_items,
            interval_submenu,
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
    let state_for_monitor = Arc::clone(&state);
    spawn_monitor_thread(state_for_monitor);

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

/// クリップボード監視スレッドの開始（モードに応じて適切な方式を選択）
fn spawn_monitor_thread(state: Arc<AppState>) {
    let monitor_mode = state.get_monitor_mode();
    let generation = state.monitor_generation.fetch_add(1, Ordering::SeqCst) + 1;

    match monitor_mode {
        MonitorMode::Polling => spawn_polling_monitor_thread(state, generation),
        #[cfg(windows)]
        MonitorMode::Event => spawn_event_monitor_thread(state, generation),
    }
}

/// ポーリング方式のクリップボード監視スレッド
fn spawn_polling_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                notification::error::show_anyhow_error("監視スレッドエラー", &e);
                return;
            }
        };

        {
            let current_text = clipboard.get_text().unwrap_or_default();
            state.set_last_processed_text(current_text);
        }

        loop {
            // 監視モード変更時にスレッドを終了（最新の世代でないなら終了）
            if state.monitor_generation.load(Ordering::SeqCst) != generation {
                break;
            }

            let interval = state.interval_ms.load(Ordering::Relaxed);
            thread::sleep(Duration::from_millis(interval));

            if state.paused.load(Ordering::Relaxed) {
                continue;
            }

            if let Ok(text) = clipboard.get_text() {
                let shared_last = state.get_last_processed_text();

                if !text.is_empty() && text != shared_last {
                    let current_mode = state.get_mode();
                    if let Some(processed) = process_clipboard(&mut clipboard, current_mode) {
                        state.set_last_processed_text(processed.clone());
                        // 通知を表示
                        show_process_notification(current_mode, &processed);
                        continue;
                    }
                }
                state.set_last_processed_text(text);
            }
        }
    });
}

/// イベント方式のクリップボード監視スレッド（Windowsのみ）
#[cfg(windows)]
fn spawn_event_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        use clipboard_win::raw::seq_num;

        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                notification::error::show_anyhow_error("監視スレッドエラー", &e);
                return;
            }
        };

        let current_text = clipboard.get_text().unwrap_or_default();
        state.set_last_processed_text(current_text);
        let mut last_seq = seq_num().map(|s| s.get()).unwrap_or(0);

        loop {
            // 監視モード変更時にスレッドを終了（最新の世代でないなら終了）
            if state.monitor_generation.load(Ordering::SeqCst) != generation {
                break;
            }

            if state.paused.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(200));
                continue;
            }

            // クリップボードのシーケンス番号をチェック
            if let Some(seq_nonzero) = seq_num() {
                let seq = seq_nonzero.get();
                if seq != last_seq {
                    last_seq = seq;

                    // テキストを取得して処理
                    if let Ok(text) = clipboard.get_text() {
                        let shared_last = state.get_last_processed_text();

                        if !text.is_empty() && text != shared_last {
                            let current_mode = state.get_mode();
                            if let Some(processed) = process_clipboard(&mut clipboard, current_mode)
                            {
                                state.set_last_processed_text(processed.clone());
                                // 通知を表示
                                show_process_notification(current_mode, &processed);
                                // シーケンス番号を更新（自分が更新したため）
                                last_seq = seq_num().map(|s| s.get()).unwrap_or(last_seq);
                                continue;
                            }
                        }
                        state.set_last_processed_text(text);
                    }
                }
            }

            // 変化がない時のCPU負荷を抑える
            thread::sleep(Duration::from_millis(100));
        }
    });
}

/// メニューイベントのハンドリング
fn handle_menu_event(
    event: MenuEvent,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard: &mut Clipboard,
    control_flow: &mut ControlFlow,
) {
    if event.id == menu.quit_item.id() {
        *control_flow = ControlFlow::Exit;
    } else if event.id == menu.pause_item.id() {
        state
            .paused
            .store(menu.pause_item.is_checked(), Ordering::Relaxed);
    } else if let Some((_, mode)) = menu
        .mode_items
        .iter()
        .chain(menu.json_format_items.iter())
        .chain(menu.json_to_yaml_items.iter())
        .chain(menu.yaml_to_json_items.iter())
        .find(|(item, _)| event.id == item.id())
    {
        update_refine(state, menu, clipboard, *mode);
    } else if let Some((_, monitor_mode)) = menu
        .monitor_mode_items
        .iter()
        .find(|(item, _)| event.id == item.id())
    {
        update_monitor_mode(state, menu, *monitor_mode);
    } else {
        for (item, ms) in &menu.interval_items {
            if event.id == item.id() {
                state.interval_ms.store(*ms, Ordering::Relaxed);
                for (it, _) in &menu.interval_items {
                    it.set_checked(false);
                }
                item.set_checked(true);
                state.save_config();
                break;
            }
        }
    }
}

/// 加工モードの更新
fn update_refine(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard: &mut Clipboard,
    mode: RefineMode,
) {
    state.set_mode(mode);

    // 通常項目
    for (item, m) in &menu.mode_items {
        item.set_checked(*m == mode);
    }
    // JSON整形
    for (item, m) in &menu.json_format_items {
        item.set_checked(*m == mode);
    }
    // JSON→YAML
    for (item, m) in &menu.json_to_yaml_items {
        item.set_checked(*m == mode);
    }
    // YAML→JSON
    for (item, m) in &menu.yaml_to_json_items {
        item.set_checked(*m == mode);
    }

    state.save_config();
    if let Some(processed) = process_clipboard(clipboard, mode) {
        state.set_last_processed_text(processed.clone());
        show_process_notification(mode, &processed);
    }
}

/// 処理完了通知を表示する
#[cfg(debug_assertions)]
fn show_process_notification(mode: RefineMode, text: &str) {
    let snippet = if text.chars().count() > 50 {
        format!("{}...", text.chars().take(47).collect::<String>())
    } else {
        text.to_string()
    };
    notification::success::show_success_notification(
        "変換完了",
        &format!("モード: {}\n内容: {}", mode.label(), snippet),
    );
}

/// 処理完了通知を表示する (リリースビルドでは何もしない)
#[cfg(not(debug_assertions))]
fn show_process_notification(_mode: RefineMode, _text: &str) {}

/// 監視モードの更新
fn update_monitor_mode(state: &Arc<AppState>, menu: &TrayMenu, monitor_mode: MonitorMode) {
    let current_mode = state.get_monitor_mode();

    // モードが変わっていない場合は何もしない
    if current_mode == monitor_mode {
        return;
    }

    // 監視モードを更新
    state.set_monitor_mode(monitor_mode);

    // メニューのチェック状態を更新
    for (item, m) in &menu.monitor_mode_items {
        item.set_checked(*m == monitor_mode);
    }

    // 監視周期メニューの有効/無効を切り替え
    #[cfg(windows)]
    match monitor_mode {
        MonitorMode::Event => menu.interval_submenu.set_enabled(false),
        MonitorMode::Polling => menu.interval_submenu.set_enabled(true),
    }

    #[cfg(not(windows))]
    match monitor_mode {
        MonitorMode::Polling => menu.interval_submenu.set_enabled(true),
    }

    state.save_config();

    // 監視スレッドを再起動（世代を更新することで旧スレッドを終了させる）
    spawn_monitor_thread(Arc::clone(state));
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
