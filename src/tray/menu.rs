use anyhow::{Context, Result};
use std::sync::Mutex;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

use super::state::{AppState, LockExt};
use crate::config::MonitorMode;
use crate::refiner::{RefineCategory, RefineMode};

/// トレイメニューの管理
pub struct TrayMenu {
    /// トレイアイコンのインスタンス。Dropされるとアイコンが消えるため、所有権を維持する必要がある。
    /// フィールド名の`_`プレフィックスは、変数が直接使用されなくても所有権を保持するために意図的に付けられていることを示す。
    pub _tray_icon: TrayIcon,
    pub quit_item: MenuItem,
    pub pause_item: CheckMenuItem,
    pub mode_items: Vec<(CheckMenuItem, RefineMode)>,
    pub line_actions_items: Vec<(CheckMenuItem, RefineMode)>,
    pub trim_items: Vec<(CheckMenuItem, RefineMode)>,
    pub escape_items: Vec<(CheckMenuItem, RefineMode)>,
    pub json_format_items: Vec<(CheckMenuItem, RefineMode)>,
    pub json_to_yaml_items: Vec<(CheckMenuItem, RefineMode)>,
    pub yaml_to_json_items: Vec<(CheckMenuItem, RefineMode)>,
    pub datetime_items: Vec<(CheckMenuItem, RefineMode)>,
    pub number_items: Vec<(CheckMenuItem, RefineMode)>,
    pub monitor_mode_items: Vec<(CheckMenuItem, MonitorMode)>,
    pub interval_submenu: Submenu,
    pub interval_items: Vec<(CheckMenuItem, u64)>,
    pub history_submenu: Submenu,
    pub history_enabled_item: CheckMenuItem,
    pub clear_history_item: MenuItem,
    pub history_records: Mutex<Vec<(tray_icon::menu::MenuId, String)>>,
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
    pub fn build(state: &AppState) -> Result<Self> {
        use std::sync::atomic::Ordering;
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
    /// [略] (元のコメントを維持)
    #[allow(clippy::type_complexity)]
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
                RefineCategory::Normal => mode_items.push((item, mode)),
                RefineCategory::LineActions => line_actions_items.push((item, mode)),
                RefineCategory::Trim => trim_items.push((item, mode)),
                RefineCategory::Escape => escape_items.push((item, mode)),
                RefineCategory::JsonFormat => json_format_items.push((item, mode)),
                RefineCategory::JsonToYaml => json_to_yaml_items.push((item, mode)),
                RefineCategory::YamlToJson => yaml_to_json_items.push((item, mode)),
                RefineCategory::Datetime => datetime_items.push((item, mode)),
                RefineCategory::Number => number_items.push((item, mode)),
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
    pub fn refresh_history(&self, state: &AppState) -> Result<()> {
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

/// トレイに表示するアイコンデータを読み込んで作成する。
pub fn create_icon() -> Result<Icon> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let img =
        image::load_from_memory(icon_bytes).context("アイコン画像のデコードに失敗しました")?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).context("アイコンデータの作成に失敗しました")
}
