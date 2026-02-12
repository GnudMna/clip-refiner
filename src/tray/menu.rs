use std::sync::Mutex;

use super::state::{AppState, LockExt};
use crate::config::MonitorMode;
use crate::refiner::{RefineCategory, RefineMode};

use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

/// 変換モードのカテゴリごとのグループ
pub struct CategoryGroup {
    /// カテゴリ用のサブメニュー
    pub submenu: Submenu,
    /// サブメニュー内に配置されるモードアイテムのリスト
    pub items: Vec<(CheckMenuItem, RefineMode)>,
    /// 所属するカテゴリ
    pub category: RefineCategory,
}

/// 変換モードメニューの構成要素
pub struct RefineMenu {
    /// 「変換モード」メインサブメニュー
    pub main_submenu: Submenu,
    /// サブメニューに属さない直下配下のモードアイテム
    pub normal_items: Vec<(CheckMenuItem, RefineMode)>,
    /// カテゴリ別のサブメニューグループ
    pub groups: Vec<CategoryGroup>,
}

impl RefineMenu {
    /// すべてのモードアイテム（CheckMenuItem と RefineMode のペア）を平坦化したイテレータとして返す。
    ///
    /// # Returns
    /// すべての `(CheckMenuItem, RefineMode)` ペアを巡回するイテレータ。
    pub fn all_items(&self) -> impl Iterator<Item = &(CheckMenuItem, RefineMode)> {
        self.normal_items
            .iter()
            .chain(self.groups.iter().flat_map(|g| g.items.iter()))
    }
}

/// 監視方式メニューの構成要素
pub struct MonitorMenu {
    pub main_submenu: Submenu,
    pub items: Vec<(CheckMenuItem, MonitorMode)>,
}

/// 監視周期メニューの構成要素
pub struct IntervalMenu {
    pub main_submenu: Submenu,
    pub items: Vec<(CheckMenuItem, u64)>,
}

/// 履歴メニューの構成要素
pub struct HistoryMenu {
    pub main_submenu: Submenu,
    pub enabled_item: CheckMenuItem,
    pub clear_item: MenuItem,
    pub records: Mutex<Vec<(tray_icon::menu::MenuId, String)>>,
}

/// 通知メニューの構成要素
pub struct NotificationMenu {
    pub main_submenu: Submenu,
    pub enabled_item: CheckMenuItem,
    pub content_submenu: Submenu,
    pub notify_mode_item: CheckMenuItem,
    pub notify_result_item: CheckMenuItem,
    pub notify_pause_item: CheckMenuItem,
}

/// トレイメニューの管理
pub struct TrayMenu {
    /// トレイアイコンのインスタンス。Dropされるとアイコンが消えるため、所有権を維持する必要がある。
    pub _tray_icon: TrayIcon,
    pub quit_item: MenuItem,
    pub pause_item: CheckMenuItem,
    pub refine: RefineMenu,
    pub monitor: MonitorMenu,
    pub interval: IntervalMenu,
    pub history: HistoryMenu,
    pub notification: NotificationMenu,
    pub shortcut_list_item: MenuItem,
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
        let show_success_notification = state.show_success_notification.load(Ordering::Relaxed);

        let refine = Self::build_refine_menu(current_mode)?;
        let monitor = Self::build_monitor_menu(current_monitor_mode)?;
        let interval = Self::build_interval_menu(current_interval, current_monitor_mode)?;
        let history = Self::build_history_menu(history_enabled)?;
        let notification = Self::build_notification_menu(
            show_success_notification,
            state.notification_notify_mode.load(Ordering::Relaxed),
            state.notification_notify_result.load(Ordering::Relaxed),
            state.notification_notify_pause.load(Ordering::Relaxed),
        )?;
        notification
            .content_submenu
            .set_enabled(show_success_notification);

        // その他のメニュー
        let pause_item =
            CheckMenuItem::new("一時停止", true, state.paused.load(Ordering::Relaxed), None);
        let shortcut_list_item = MenuItem::new("ショートカット一覧", true, None);
        let quit_item = MenuItem::new("終了", true, None);

        // メインメニューの組み立て
        let tray_menu = Menu::new();
        tray_menu
            .append_items(&[
                &refine.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &monitor.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &interval.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &history.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &notification.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &shortcut_list_item as &dyn tray_icon::menu::IsMenuItem,
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

        let this = Self {
            _tray_icon,
            quit_item,
            pause_item,
            refine,
            monitor,
            interval,
            history,
            notification,
            shortcut_list_item,
        };

        // カテゴリラベルの初期更新
        this.refresh_category_labels(current_mode);

        Ok(this)
    }

    /// 変換モードメニューを構築する
    ///
    /// # Arguments
    /// * `current_mode` - 現在選択されている変換モード。
    ///
    /// # Returns
    /// 成功した場合は `RefineMenu` インスタンスを返し、失敗した場合は `Err` を返す。
    fn build_refine_menu(current_mode: RefineMode) -> Result<RefineMenu> {
        use std::collections::HashMap;

        let mut items_by_category: HashMap<RefineCategory, Vec<(CheckMenuItem, RefineMode)>> =
            HashMap::new();

        for &mode in RefineMode::variants() {
            let item = CheckMenuItem::new(mode.label(), true, mode == current_mode, None);
            items_by_category
                .entry(mode.category())
                .or_default()
                .push((item, mode));
        }

        let normal_items = items_by_category
            .remove(&RefineCategory::Normal)
            .unwrap_or_default();

        // 期待されるサブメニューの順序
        let category_order = [
            RefineCategory::UrlActions,
            RefineCategory::Path,
            RefineCategory::LineActions,
            RefineCategory::Trim,
            RefineCategory::Escape,
            RefineCategory::JsonFormat,
            RefineCategory::ToJson,
            RefineCategory::ToYaml,
            RefineCategory::Datetime,
            RefineCategory::Number,
        ];

        let mut groups = Vec::new();
        for &category in &category_order {
            if let Some(items) = items_by_category.remove(&category) {
                let submenu = Submenu::with_items(
                    category.label(),
                    true,
                    &items
                        .iter()
                        .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                        .collect::<Vec<_>>(),
                )?;
                groups.push(CategoryGroup {
                    submenu,
                    items,
                    category,
                });
            }
        }

        // メインの変換モードメニュー組み立て
        let mut mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();

        // カテゴリグループの追加
        for group in &groups {
            // 特定のカテゴリ以外を最初に追加
            if group.category != RefineCategory::Datetime
                && group.category != RefineCategory::Number
            {
                mode_menu_items.push(&group.submenu);
            }
        }

        // 通常アイテムと遅延追加カテゴリの配置
        for (item, mode) in &normal_items {
            mode_menu_items.push(item);
            // 特定の通常項目の後にカテゴリを配置
            if *mode == RefineMode::ExcelToMarkdown {
                for group in &groups {
                    if group.category == RefineCategory::Datetime
                        || group.category == RefineCategory::Number
                    {
                        mode_menu_items.push(&group.submenu);
                    }
                }
            }
        }

        let main_submenu = Submenu::with_items("変換モード", true, &mode_menu_items)
            .context("変換モードメニューの作成に失敗しました")?;

        Ok(RefineMenu {
            main_submenu,
            normal_items,
            groups,
        })
    }

    /// 所属カテゴリに基づいてサブメニューのラベルを更新する
    ///
    /// 選択されているモードが属するカテゴリのサブメニューに「✓」プレフィックスを付与し、
    /// それ以外のサブメニューからは削除する。
    ///
    /// # Arguments
    /// * `current_mode` - 現在選択されている変換モード。
    pub fn refresh_category_labels(&self, current_mode: RefineMode) {
        let current_category = current_mode.category();

        for group in &self.refine.groups {
            let prefix = if current_category == group.category {
                "✓"
            } else {
                ""
            };
            group
                .submenu
                .set_text(format!("{}{}", prefix, group.category.label()));
        }
    }

    /// 監視方式メニューを構築する
    ///
    /// # Arguments
    /// * `current_monitor_mode` - 現在選択されている監視方式。
    ///
    /// # Returns
    /// 成功した場合は `MonitorMenu` インスタンスを返し、失敗した場合は `Err` を返す。
    fn build_monitor_menu(current_monitor_mode: MonitorMode) -> Result<MonitorMenu> {
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
        let main_submenu = Submenu::with_items("監視方式", true, &monitor_mode_menu_items)
            .context("監視方式メニューの作成に失敗しました")?;

        Ok(MonitorMenu {
            main_submenu,
            items: monitor_mode_items,
        })
    }

    /// 監視周期メニューを構築する
    ///
    /// # Arguments
    /// * `current_interval` - 現在設定されている監視間隔（ミリ秒）
    /// * `monitor_mode` - 現在の監視方式（イベントモード時はメニューを無効化するため）
    ///
    /// # Returns
    /// 成功した場合は `IntervalMenu` インスタンスを返し、失敗した場合は `Err` を返す。
    fn build_interval_menu(
        current_interval: u64,
        monitor_mode: MonitorMode,
    ) -> Result<IntervalMenu> {
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
        let main_submenu = Submenu::with_items("監視周期", true, &interval_menu_items)
            .context("監視周期メニューの作成に失敗しました")?;

        // イベントモードの場合は監視周期を無効化
        #[cfg(windows)]
        if monitor_mode == MonitorMode::Event {
            main_submenu.set_enabled(false);
        }

        Ok(IntervalMenu {
            main_submenu,
            items: interval_items,
        })
    }

    /// 履歴メニューの基本構造を構築する
    ///
    /// # Arguments
    /// * `history_enabled` - 履歴機能が有効かどうか。
    ///
    /// # Returns
    /// 成功した場合は `HistoryMenu` インスタンスを返し、失敗した場合は `Err` を返す。
    fn build_history_menu(history_enabled: bool) -> Result<HistoryMenu> {
        let enabled_item = CheckMenuItem::new("履歴機能を有効にする", true, history_enabled, None);
        let clear_item = MenuItem::new("履歴をクリア", true, None);
        let main_submenu = Submenu::new("履歴", true);
        let records = Mutex::new(Vec::new());

        // 初期の履歴メニュー構築
        main_submenu.append_items(&[
            &enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            &clear_item as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        Ok(HistoryMenu {
            main_submenu,
            enabled_item,
            clear_item,
            records,
        })
    }

    /// 通知メニューを構築する
    ///
    /// # Arguments
    /// * `enabled` - 成功通知を有効にするかどうか。
    /// * `notify_mode` - 通知にモード変化を表示するかどうか。
    /// * `notify_result` - 通知に加工結果を表示するかどうか。
    ///
    /// # Returns
    /// 成功した場合は `NotificationMenu` インスタンスを返し、失敗した場合は `Err` を返す。
    fn build_notification_menu(
        enabled: bool,
        notify_mode: bool,
        notify_result: bool,
        notify_pause: bool,
    ) -> Result<NotificationMenu> {
        let enabled_item = CheckMenuItem::new("成功通知を有効化", true, enabled, None);
        let notify_mode_item = CheckMenuItem::new("モード変更を通知", true, notify_mode, None);
        let notify_result_item = CheckMenuItem::new("変換結果を通知", true, notify_result, None);
        let notify_pause_item = CheckMenuItem::new("一時停止を通知", true, notify_pause, None);

        let content_submenu = Submenu::with_items(
            "通知内容",
            true,
            &[
                &notify_mode_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_result_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_pause_item as &dyn tray_icon::menu::IsMenuItem,
            ],
        )?;

        let main_submenu = Submenu::with_items(
            "通知",
            true,
            &[
                &enabled_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &content_submenu as &dyn tray_icon::menu::IsMenuItem,
            ],
        )?;

        Ok(NotificationMenu {
            main_submenu,
            enabled_item,
            content_submenu,
            notify_mode_item,
            notify_result_item,
            notify_pause_item,
        })
    }

    /// 履歴メニューの内容を最新の状態に更新する
    ///
    /// # Arguments
    /// * `state` - 現在のアプリケーション状態。
    pub fn refresh_history(&self, state: &AppState) -> Result<()> {
        let history = state.get_history();
        let mut records = self.history.records.lock_ignore_poison();
        records.clear();

        // 既存の履歴アイテムをクリア（有効化スイッチと区切り線以外）
        for _ in 0..self.history.main_submenu.items().len() {
            self.history.main_submenu.remove_at(0);
        }

        // 基本部分を再構築
        self.history.main_submenu.append_items(&[
            &self.history.enabled_item as &dyn tray_icon::menu::IsMenuItem,
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
                self.history
                    .main_submenu
                    .append_items(&[&item as &dyn tray_icon::menu::IsMenuItem])?;
            }
            self.history.main_submenu.append_items(&[
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem
            ])?;
        }

        self.history
            .main_submenu
            .append_items(&[&self.history.clear_item as &dyn tray_icon::menu::IsMenuItem])?;

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
