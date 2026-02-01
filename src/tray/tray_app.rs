use std::sync::{
    Arc, Mutex, MutexGuard,
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
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};

/// アプリケーション内でのカスタムイベント
enum AppEvent {
    /// 履歴メニューの更新要求
    RefreshHistory,
}

/// 履歴の最大保持数
const HISTORY_LIMIT: usize = 10;

/// Mutexのポイズニングを無視してロックを取得するための拡張トレイト
trait LockExt<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// アプリケーション内で共有されるミュータブルな状態
///
/// Mutexのロックに失敗した場合（ポイズニング）、パニックせずに以前の値を返して
/// アプリケーションの実行を継続する方針をとる。
struct AppState {
    /// 現在選択されている加工モード
    mode: Mutex<RefineMode>,
    /// 監視が一時停止されているかどうか
    paused: AtomicBool,
    /// 監視方式（Polling または Event）
    monitor_mode: Mutex<MonitorMode>,
    /// 監視スレッドの世代管理用カウンタ。設定変更時に古いスレッドを破棄するために使用
    monitor_generation: AtomicU64,
    /// ポーリング時の監視間隔（ミリ秒）
    interval_ms: AtomicU64,
    /// 二重加工を防止するために保持される、最後に加工されたテキスト
    last_processed_text: Mutex<String>,
    /// 履歴機能が有効かどうか
    history_enabled: AtomicBool,
    /// クリップボード履歴（最大10件）
    history: Mutex<Vec<String>>,
    /// イベントループへのプロキシ。別スレッドからUIイベント（例: 履歴更新）を送信するために使用される。
    proxy: EventLoopProxy<AppEvent>,
}

impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Returns
    /// * `Self` - 新しく生成された `AppState` インスタンス。
    fn new(proxy: EventLoopProxy<AppEvent>) -> Self {
        let config = AppConfig::load();
        Self {
            mode: Mutex::new(config.mode),
            paused: AtomicBool::new(false),
            monitor_mode: Mutex::new(config.monitor_mode),
            monitor_generation: AtomicU64::new(0),
            interval_ms: AtomicU64::new(config.interval_ms),
            last_processed_text: Mutex::new(String::new()),
            history_enabled: AtomicBool::new(config.history_enabled),
            history: Mutex::new(Vec::new()),
            proxy,
        }
    }

    /// 現在の設定をファイルへ保存する。
    fn save_config(&self) {
        let config = AppConfig {
            mode: self.get_mode(),
            interval_ms: self.interval_ms.load(Ordering::Relaxed),
            monitor_mode: self.get_monitor_mode(),
            history_enabled: self.history_enabled.load(Ordering::Relaxed),
        };
        if let Err(e) = config.save() {
            eprintln!("設定の保存に失敗: {}", e);
        }
    }

    /// 現在の `RefineMode` をスレッドセーフに取得する。
    ///
    /// # Returns
    /// * `RefineMode` - 現在設定されている `RefineMode`。
    fn get_mode(&self) -> RefineMode {
        *self.mode.lock_ignore_poison()
    }

    /// `RefineMode` をスレッドセーフに設定する。
    ///
    /// # Arguments
    /// * `mode` - 新しく設定する `RefineMode`。
    fn set_mode(&self, mode: RefineMode) {
        *self.mode.lock_ignore_poison() = mode;
    }

    /// 現在の `MonitorMode` をスレッドセーフに取得する。
    ///
    /// # Returns
    /// * `MonitorMode` - 現在設定されている `MonitorMode`。
    fn get_monitor_mode(&self) -> MonitorMode {
        *self.monitor_mode.lock_ignore_poison()
    }

    /// `MonitorMode` をスレッドセーフに設定する。
    ///
    /// # Arguments
    /// * `mode` - 新しく設定する `MonitorMode`。
    fn set_monitor_mode(&self, mode: MonitorMode) {
        *self.monitor_mode.lock_ignore_poison() = mode;
    }

    /// 加工済みの最新テキストをスレッド安全に取得する
    ///
    /// # Returns
    /// * `String` - 最後に加工されたテキストのクローン。
    fn get_last_processed_text(&self) -> String {
        self.last_processed_text.lock_ignore_poison().clone()
    }

    /// 加工済みの最新テキストをスレッド安全に更新する
    ///
    /// # Arguments
    /// * `text` - 新しく設定する、加工済みのテキスト。
    fn set_last_processed_text(&self, text: String) {
        *self.last_processed_text.lock_ignore_poison() = text;
    }

    /// 履歴を取得する
    fn get_history(&self) -> Vec<String> {
        self.history.lock_ignore_poison().clone()
    }

    /// 履歴をクリアする
    fn clear_history(&self) {
        self.history.lock_ignore_poison().clear();
    }

    /// 履歴にテキストを追加する。
    /// すでに存在する場合は最上位に移動させ、最大10件を保持する。
    fn add_to_history(&self, text: String) {
        if text.trim().is_empty() {
            return;
        }

        let mut history = self.history.lock_ignore_poison();

        // 二重登録防止: すでに存在すれば削除して最上位へ
        if let Some(pos) = history.iter().position(|x| x == &text) {
            history.remove(pos);
        }

        history.insert(0, text);

        // 最大10件
        if history.len() > HISTORY_LIMIT {
            history.truncate(HISTORY_LIMIT);
        }

        let _ = self.proxy.send_event(AppEvent::RefreshHistory);
    }
}

/// トレイメニューの管理
struct TrayMenu {
    /// トレイアイコンのインスタンス。Dropされるとアイコンが消えるため、所有権を維持する必要がある。
    /// フィールド名の`_`プレフィックスは、変数が直接使用されなくても所有権を保持するために意図的に付けられていることを示す。
    _tray_icon: TrayIcon,
    quit_item: MenuItem,
    pause_item: CheckMenuItem,
    mode_items: Vec<(CheckMenuItem, RefineMode)>,
    line_actions_items: Vec<(CheckMenuItem, RefineMode)>,
    trim_items: Vec<(CheckMenuItem, RefineMode)>,
    escape_items: Vec<(CheckMenuItem, RefineMode)>,
    json_format_items: Vec<(CheckMenuItem, RefineMode)>,
    json_to_yaml_items: Vec<(CheckMenuItem, RefineMode)>,
    yaml_to_json_items: Vec<(CheckMenuItem, RefineMode)>,
    datetime_items: Vec<(CheckMenuItem, RefineMode)>,
    number_items: Vec<(CheckMenuItem, RefineMode)>,
    monitor_mode_items: Vec<(CheckMenuItem, MonitorMode)>,
    interval_submenu: Submenu,
    interval_items: Vec<(CheckMenuItem, u64)>,
    history_submenu: Submenu,
    history_enabled_item: CheckMenuItem,
    clear_history_item: MenuItem,
    history_records: Mutex<Vec<(tray_icon::menu::MenuId, String)>>,
}

impl TrayMenu {
    /// トレイアイコンのメニューを構築する。
    ///
    /// 現在のアプリケーション状態に基づいて、各種メニュー項目（変換モード、監視方式、監視周期など）を作成し、
    /// トレイアイコンに設定する。
    ///
    /// # Arguments
    /// * `state` - 現在のアプリケーション状態。メニューの初期状態の決定に使用される。
    ///
    /// # Returns
    /// メニューの構築に成功した場合は`Ok(Self)`、失敗した場合は`Err`を返す。
    fn build(state: &AppState) -> Result<Self> {
        let current_mode = state.get_mode();
        let current_interval = state.interval_ms.load(Ordering::Relaxed);
        let current_monitor_mode = state.get_monitor_mode();
        let history_enabled = state.history_enabled.load(Ordering::Relaxed);

        let (
            refine_submenu,
            mode_items,
            line_actions_items,
            trim_items,
            escape_items,
            json_format_items,
            json_to_yaml_items,
            yaml_to_json_items,
            datetime_items,
            number_items,
        ) = Self::build_refine_menu(current_mode)?;

        let (monitor_mode_submenu, monitor_mode_items) =
            Self::build_monitor_menu(current_monitor_mode)?;

        let (interval_submenu, interval_items) =
            Self::build_interval_menu(current_interval, current_monitor_mode)?;

        let (history_submenu, history_enabled_item, clear_history_item, history_records) =
            Self::build_history_menu(history_enabled)?;

        // 一時停止・終了メニュー
        let pause_item =
            CheckMenuItem::new("一時停止", true, state.paused.load(Ordering::Relaxed), None);
        let quit_item = MenuItem::new("終了", true, None);

        // メインメニューの組み立て
        let tray_menu = Menu::new();
        tray_menu
            .append_items(&[
                &refine_submenu as &dyn tray_icon::menu::IsMenuItem,
                &monitor_mode_submenu as &dyn tray_icon::menu::IsMenuItem,
                &interval_submenu as &dyn tray_icon::menu::IsMenuItem,
                &history_submenu as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &pause_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &quit_item as &dyn tray_icon::menu::IsMenuItem,
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
            line_actions_items,
            trim_items,
            escape_items,
            json_format_items,
            json_to_yaml_items,
            yaml_to_json_items,
            datetime_items,
            number_items,
            monitor_mode_items,
            interval_submenu,
            interval_items,
            history_submenu,
            history_enabled_item,
            clear_history_item,
            history_records,
        })
    }

    /// 変換モードメニューを構築する
    ///
    /// # Arguments
    /// * `current_mode` - 現在選択されている変換モード
    ///
    /// # Returns
    /// * `Submenu` - 変換モードのサブメニュー
    /// * `Vec<(CheckMenuItem, RefineMode)>` - 全モードのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - LineActionsカテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - Trimカテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - Escapeカテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - JSON整形カテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - JSON→YAMLカテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - YAML→JSONカテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - Datetimeカテゴリのアイテムリスト
    /// * `Vec<(CheckMenuItem, RefineMode)>` - Numberカテゴリのアイテムリスト
    fn build_refine_menu(
        current_mode: RefineMode,
    ) -> Result<(
        Submenu,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
        Vec<(CheckMenuItem, RefineMode)>,
    )> {
        let mut line_actions_items = Vec::new();
        let mut trim_items = Vec::new();
        let mut escape_items = Vec::new();
        let mut json_format_items = Vec::new();
        let mut json_to_yaml_items = Vec::new();
        let mut yaml_to_json_items = Vec::new();
        let mut datetime_items = Vec::new();
        let mut number_items = Vec::new();
        let mut mode_items = Vec::new();

        for &mode in RefineMode::variants() {
            let item = CheckMenuItem::new(mode.label(), true, mode == current_mode, None);
            match mode.category() {
                crate::refiner::RefineCategory::Normal => mode_items.push((item, mode)),
                crate::refiner::RefineCategory::LineActions => {
                    line_actions_items.push((item, mode))
                }
                crate::refiner::RefineCategory::Trim => trim_items.push((item, mode)),
                crate::refiner::RefineCategory::Escape => escape_items.push((item, mode)),
                crate::refiner::RefineCategory::JsonFormat => json_format_items.push((item, mode)),
                crate::refiner::RefineCategory::JsonToYaml => json_to_yaml_items.push((item, mode)),
                crate::refiner::RefineCategory::YamlToJson => yaml_to_json_items.push((item, mode)),
                crate::refiner::RefineCategory::Datetime => datetime_items.push((item, mode)),
                crate::refiner::RefineCategory::Number => number_items.push((item, mode)),
            }
        }

        // サブメニューの作成
        let line_actions_submenu = Submenu::with_items(
            "行操作",
            true,
            &line_actions_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;
        let trim_submenu = Submenu::with_items(
            "トリム",
            true,
            &trim_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;
        let escape_submenu = Submenu::with_items(
            "エスケープ",
            true,
            &escape_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;
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
        let datetime_submenu = Submenu::with_items(
            "日時変換",
            true,
            &datetime_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;
        let number_submenu = Submenu::with_items(
            "数値変換",
            true,
            &number_items
                .iter()
                .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                .collect::<Vec<_>>(),
        )?;

        // メインの変換モードメニュー組み立て
        let mut mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, mode) in &mode_items {
            mode_menu_items.push(item);
            // 特定の項目の後にサブメニューを配置
            if *mode == RefineMode::RemoveUtm {
                mode_menu_items.push(&line_actions_submenu);
                mode_menu_items.push(&trim_submenu);
                mode_menu_items.push(&escape_submenu);
                mode_menu_items.push(&json_format_submenu);
                mode_menu_items.push(&json_to_yaml_submenu);
                mode_menu_items.push(&yaml_to_json_submenu);
            } else if *mode == RefineMode::MarkdownToHtml {
                mode_menu_items.push(&datetime_submenu);
                mode_menu_items.push(&number_submenu);
            }
        }

        let refine_submenu = Submenu::with_items("変換モード", true, &mode_menu_items)
            .context("変換モードメニューの作成に失敗しました")?;

        Ok((
            refine_submenu,
            mode_items,
            line_actions_items,
            trim_items,
            escape_items,
            json_format_items,
            json_to_yaml_items,
            yaml_to_json_items,
            datetime_items,
            number_items,
        ))
    }

    /// 監視方式メニューを構築する
    ///
    /// # Arguments
    /// * `current_monitor_mode` - 現在選択されている監視方式
    ///
    /// # Returns
    /// * `Submenu` - 監視方式のサブメニュー
    /// * `Vec<(CheckMenuItem, MonitorMode)>` - 監視方式のアイテムリスト
    fn build_monitor_menu(
        current_monitor_mode: MonitorMode,
    ) -> Result<(Submenu, Vec<(CheckMenuItem, MonitorMode)>)> {
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

        Ok((monitor_mode_submenu, monitor_mode_items))
    }

    /// 監視周期メニューを構築する
    ///
    /// # Arguments
    /// * `current_interval` - 現在設定されている監視間隔（ミリ秒）
    /// * `monitor_mode` - 現在の監視方式（イベントモード時はメニューを無効化するため）
    ///
    /// # Returns
    /// * `Submenu` - 監視周期のサブメニュー
    /// * `Vec<(CheckMenuItem, u64)>` - 監視周期のアイテムリスト
    fn build_interval_menu(
        current_interval: u64,
        monitor_mode: MonitorMode,
    ) -> Result<(Submenu, Vec<(CheckMenuItem, u64)>)> {
        let interval_items = vec![
            (
                CheckMenuItem::new("0.5秒", true, current_interval == 500, None),
                500u64,
            ),
            (
                CheckMenuItem::new("1秒", true, current_interval == 1000, None),
                1000u64,
            ),
            (
                CheckMenuItem::new("2秒", true, current_interval == 2000, None),
                2000u64,
            ),
            (
                CheckMenuItem::new("5秒", true, current_interval == 5000, None),
                5000u64,
            ),
        ];

        let mut interval_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, _) in &interval_items {
            interval_menu_items.push(item);
        }
        let interval_submenu = Submenu::with_items("監視周期", true, &interval_menu_items)
            .context("監視周期メニューの作成に失敗しました")?;

        // イベントモードの場合は監視周期を無効化
        #[cfg(windows)]
        if monitor_mode == MonitorMode::Event {
            interval_submenu.set_enabled(false);
        }

        Ok((interval_submenu, interval_items))
    }

    /// 履歴メニューの基本構造を構築する
    ///
    /// # Arguments
    /// * `history_enabled` - 履歴機能が有効かどうか
    ///
    /// # Returns
    /// * `Submenu` - 履歴のサブメニュー
    /// * `CheckMenuItem` - 履歴有効化のチェック項目
    /// * `MenuItem` - 履歴クリア項目
    /// * `Mutex<Vec<(MenuId, String)>>` - 動的に更新される履歴レコードのコンテナ
    fn build_history_menu(
        history_enabled: bool,
    ) -> Result<(
        Submenu,
        CheckMenuItem,
        MenuItem,
        Mutex<Vec<(tray_icon::menu::MenuId, String)>>,
    )> {
        let history_enabled_item =
            CheckMenuItem::new("履歴機能を有効にする", true, history_enabled, None);
        let clear_history_item = MenuItem::new("履歴をクリア", true, None);
        let history_submenu = Submenu::new("履歴", true);
        let history_records = Mutex::new(Vec::new());

        // 初期の履歴メニュー構築
        history_submenu.append_items(&[
            &history_enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            &clear_history_item as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        Ok((
            history_submenu,
            history_enabled_item,
            clear_history_item,
            history_records,
        ))
    }

    /// 履歴メニューの内容を最新の状態に更新する
    fn refresh_history(&self, state: &AppState) -> Result<()> {
        let history = state.get_history();
        let mut records = self.history_records.lock_ignore_poison();
        records.clear();

        // 既存の履歴アイテムをクリア（有効化スイッチと区切り線以外）
        for _ in 0..self.history_submenu.items().len() {
            self.history_submenu.remove_at(0);
        }

        // 基本部分を再構築
        self.history_submenu.append_items(&[
            &self.history_enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        if !history.is_empty() {
            for text in history {
                let label = if text.chars().count() > 30 {
                    format!("{}...", text.chars().take(27).collect::<String>())
                } else {
                    text.clone()
                };
                let item = MenuItem::new(label, true, None);
                records.push((item.id().clone(), text));
                self.history_submenu
                    .append_items(&[&item as &dyn tray_icon::menu::IsMenuItem])?;
            }
            self.history_submenu.append_items(&[
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem
            ])?;
        }

        self.history_submenu
            .append_items(&[&self.clear_history_item as &dyn tray_icon::menu::IsMenuItem])?;

        Ok(())
    }
}

/// アプリケーションのメインループを開始する。
///
/// この関数はイベントループを初期化し、トレイアイコンとメニューを設定する。
/// クリップボード監視用の別スレッドを起動し、メニューからのイベントを待ち受ける。
/// イベントループはアプリケーションが終了するまでブロックされる。
///
/// # Returns
/// イベントループの開始に失敗した場合などに`Err`を返す。
pub fn run_loop() -> Result<()> {
    let event_loop = create_event_loop();
    let proxy = event_loop.create_proxy();
    let state = Arc::new(AppState::new(proxy.clone()));
    let menu = TrayMenu::build(&state)?;

    // 初期状態で履歴メニューを更新
    menu.refresh_history(&state)?;

    // クリップボード監視スレッドの開始
    let state_for_monitor = Arc::clone(&state);
    spawn_monitor_thread(state_for_monitor);

    let menu_channel = MenuEvent::receiver();
    let mut clipboard = init_clipboard()?;

    // イベントループの実行
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            tao::event::Event::UserEvent(AppEvent::RefreshHistory) => {
                let _ = menu.refresh_history(&state);
            }
            _ => {}
        }

        if let Ok(event) = menu_channel.try_recv() {
            handle_menu_event(event, &menu, &state, &mut clipboard, control_flow);
        }
    })
}

/// プラットフォームに応じたイベントループを作成する
///
/// Windows環境では `with_any_thread(true)` を設定し、
/// メインスレッド以外でもイベントループに関連する操作を行えるようにする。
fn create_event_loop() -> EventLoop<AppEvent> {
    #[cfg(windows)]
    return EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    #[cfg(not(windows))]
    return EventLoopBuilder::<AppEvent>::with_user_event().build();
}

/// クリップボード監視スレッドを開始する。
///
/// 現在の監視モード設定（ポーリングまたはイベント）に応じて、適切な監視スレッドを起動する。
/// スレッドの世代管理を行い、設定変更時に古いスレッドが自動的に終了するようにする。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
fn spawn_monitor_thread(state: Arc<AppState>) {
    let monitor_mode = state.get_monitor_mode();
    let generation = state.monitor_generation.fetch_add(1, Ordering::SeqCst) + 1;

    match monitor_mode {
        MonitorMode::Polling => spawn_polling_monitor_thread(state, generation),
        #[cfg(windows)]
        MonitorMode::Event => spawn_event_monitor_thread(state, generation),
    }
}

/// クリップボードの更新を検知し、必要であれば加工処理を行う
///
/// # Arguments
/// * `clipboard` - クリップボードのインスタンス
/// * `state` - アプリケーションの状態
///
/// # Returns
/// 加工が実行され、クリップボードへの書き込みも行われた場合は `true`、それ以外の場合は `false` を返す。
fn handle_clipboard_update(clipboard: &mut Clipboard, state: &Arc<AppState>) -> bool {
    if let Ok(text) = clipboard.get_text() {
        let shared_last = state.get_last_processed_text();

        if !text.is_empty() && text != shared_last {
            let current_mode = state.get_mode();
            if let Some(processed) = process_clipboard(clipboard, current_mode) {
                state.set_last_processed_text(processed.clone());
                show_process_notification(current_mode, &processed);

                if state.history_enabled.load(Ordering::Relaxed) {
                    state.add_to_history(processed);
                    let _ = state.proxy.send_event(AppEvent::RefreshHistory);
                }
                return true;
            }

            if state.history_enabled.load(Ordering::Relaxed) {
                state.add_to_history(text.clone());
                let _ = state.proxy.send_event(AppEvent::RefreshHistory);
            }
        }
        state.set_last_processed_text(text);
    }
    false // 加工されなかった
}

/// ポーリング方式でクリップボードを監視するスレッドを開始する。
///
/// 一定間隔でクリップボードの内容を確認し、変更があった場合に加工処理を呼び出す。
/// スレッドは、監視方式が変更される（世代が古くなる）か、アプリケーションが終了するまで実行を続ける。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `generation` - このスレッドの世代番号。
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

            handle_clipboard_update(&mut clipboard, &state);
        }
    });
}

/// イベント方式でクリップボードを監視するスレッドを開始する（Windows限定）。
///
/// OSのクリップボード更新イベントをリッスンし、変更があった場合に加工処理を呼び出す。
/// ポーリング方式に比べてCPU負荷が低い。
/// スレッドは、監視方式が変更される（世代が古くなる）か、アプリケーションが終了するまで実行を続ける。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `generation` - このスレッドの世代番号。
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

                    // クリップボードの更新を処理し、加工が行われたかチェック
                    if handle_clipboard_update(&mut clipboard, &state) {
                        // 加工が実行された場合、クリップボードが変更されたのでシーケンス番号を再取得して更新
                        last_seq = seq_num().map(|s| s.get()).unwrap_or(last_seq);
                    }
                }
            }

            // 変化がない時のCPU負荷を抑える
            thread::sleep(Duration::from_millis(100));
        }
    });
}

/// トレイアイコンメニューから受信したイベントを処理する。
///
/// 各メニュー項目（終了、一時停止、モード変更など）に対応するアクションを実行する。
///
/// # Arguments
/// * `event` - 受信したメニューイベント。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
/// * `clipboard` - クリップボードのインスタンス。
/// * `control_flow` - イベントループの制御フラグ。
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
    } else if event.id == menu.history_enabled_item.id() {
        let enabled = menu.history_enabled_item.is_checked();
        state.history_enabled.store(enabled, Ordering::Relaxed);
        state.save_config();
        let _ = menu.refresh_history(state);
    } else if event.id == menu.clear_history_item.id() {
        state.clear_history();
        state.save_config();
        let _ = menu.refresh_history(state);
    } else if let Some((_, text)) = menu
        .history_records
        .lock_ignore_poison()
        .iter()
        .find(|(id, _)| event.id == *id)
    {
        match Clipboard::new() {
            Ok(mut cb) => {
                if let Err(e) = cb.set_text(text.clone()) {
                    notification::error::show_anyhow_error(
                        "クリップボード設定エラー",
                        &anyhow::anyhow!(e),
                    );
                } else {
                    state.set_last_processed_text(text.clone());
                    notification::success::show_success_debug_notification(
                        "履歴から復元",
                        "クリップボードにコピーしました",
                    );
                }
            }
            Err(e) => notification::error::show_anyhow_error(
                "クリップボード初期化エラー",
                &anyhow::anyhow!(e),
            ),
        }
    } else if let Some((_, mode)) = menu
        .mode_items // 全てのモード関連アイテムをチェーンして検索
        .iter()
        .chain(menu.line_actions_items.iter())
        .chain(menu.trim_items.iter())
        .chain(menu.escape_items.iter())
        .chain(menu.json_format_items.iter())
        .chain(menu.json_to_yaml_items.iter())
        .chain(menu.yaml_to_json_items.iter())
        .chain(menu.datetime_items.iter())
        .chain(menu.number_items.iter())
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

/// 選択された加工モードをアプリケーションの状態に反映し、UIを更新する。
///
/// 新しいモードを状態に保存し、すべてのモード選択メニューのチェック状態を更新する。
/// 設定を永続化し、即座に現在のクリップボード内容に対して新しい加工モードを試す。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `menu` - トレイメニューのインスタンス。
/// * `clipboard` - クリップボードのインスタンス。
/// * `mode` - 新しく選択された加工モード。
fn update_refine(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard: &mut Clipboard,
    mode: RefineMode,
) {
    state.set_mode(mode);

    // すべてのモードアイテムをイテレートして、選択されたモードのチェック状態を更新
    menu.mode_items
        .iter()
        .chain(menu.line_actions_items.iter())
        .chain(menu.trim_items.iter())
        .chain(menu.escape_items.iter())
        .chain(menu.json_format_items.iter())
        .chain(menu.json_to_yaml_items.iter())
        .chain(menu.yaml_to_json_items.iter())
        .chain(menu.datetime_items.iter())
        .chain(menu.number_items.iter())
        .for_each(|(item, m)| item.set_checked(*m == mode));

    state.save_config();
    if let Some(processed) = process_clipboard(clipboard, mode) {
        state.set_last_processed_text(processed.clone());
        show_process_notification(mode, &processed);
    }
}

/// 処理完了通知を表示する
///
/// # Arguments
/// * `mode` - 実行された `RefineMode`。
/// * `text` - 加工後のテキスト。
#[cfg(debug_assertions)]
fn show_process_notification(mode: RefineMode, text: &str) {
    let snippet = if text.chars().count() > 50 {
        format!("{}...", text.chars().take(47).collect::<String>())
    } else {
        text.to_string()
    };
    notification::success::show_success_debug_notification(
        "変換完了",
        &format!("モード: {}\n内容: {}", mode.label(), snippet),
    );
}

/// 処理完了通知を表示する (リリースビルドでは何もしない)
///
/// # Arguments
/// * `_mode` - 実行された `RefineMode` (未使用)。
/// * `_text` - 加工後のテキスト (未使用)。
#[cfg(not(debug_assertions))]
fn show_process_notification(_mode: RefineMode, _text: &str) {}

/// 監視方式（ポーリング/イベント）を切り替える。
///
/// 新しい監視方式を状態に保存し、メニューのチェック状態を更新する。
/// 方式の変更に応じて、監視周期メニューの有効/無効を切り替える。
/// 最後に、新しい方式で動作する監視スレッドを再起動する。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `menu` - トレイメニューのインスタンス。
/// * `monitor_mode` - 新しく選択された監視方式。
fn update_monitor_mode(state: &Arc<AppState>, menu: &TrayMenu, monitor_mode: MonitorMode) {
    // モードが変わっていない場合は何もしない
    if state.get_monitor_mode() == monitor_mode {
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

/// クリップボード機能へのアクセスを初期化する。
///
/// # Returns
/// 初期化に成功した場合は`Ok(Clipboard)`、失敗した場合は`Err`を返す。
fn init_clipboard() -> Result<Clipboard> {
    Clipboard::new().context("クリップボードのアクセスに失敗しました")
}

/// トレイに表示するアイコンデータを読み込んで作成する。
///
/// アセットファイルからアイコン画像を読み込み、OSのトレイアイコン形式に変換する。
///
/// # Returns
/// アイコンの作成に成功した場合は`Ok(Icon)`、失敗した場合は`Err`を返す。
fn create_icon() -> Result<Icon> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let img =
        image::load_from_memory(icon_bytes).context("アイコン画像のデコードに失敗しました")?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).context("アイコンデータの作成に失敗しました")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_helpers() {
        let state = AppState {
            mode: Mutex::new(RefineMode::Trim),
            paused: AtomicBool::new(false),
            monitor_mode: Mutex::new(MonitorMode::Polling),
            monitor_generation: AtomicU64::new(0),
            interval_ms: AtomicU64::new(1000),
            last_processed_text: Mutex::new(String::new()),
            history_enabled: AtomicBool::new(false),
            history: Mutex::new(Vec::new()),
            proxy: create_event_loop().create_proxy(),
        };

        assert_eq!(state.get_mode(), RefineMode::Trim);
        state.set_mode(RefineMode::UrlEncode);
        assert_eq!(state.get_mode(), RefineMode::UrlEncode);

        assert_eq!(state.get_last_processed_text(), "");
        state.set_last_processed_text("hello".to_string());
        assert_eq!(state.get_last_processed_text(), "hello");

        assert_eq!(state.get_monitor_mode(), MonitorMode::Polling);

        state.interval_ms.store(2000, Ordering::Relaxed);
        assert_eq!(state.interval_ms.load(Ordering::Relaxed), 2000);

        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }
}
