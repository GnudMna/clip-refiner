use std::sync::Mutex;

use super::state::{AppState, LockExt};
use crate::config::MonitorMode;
use crate::refiner::{RefineCategory, RefineMode};
use strum::IntoEnumIterator;

use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

// ======================================================================
// メニュー構造体定義
// ======================================================================
/// 変換モードをカテゴリごとにグループ化して管理する構造体
pub struct CategoryGroup {
    /// カテゴリ用のサブメニューインスタンス
    pub submenu: Submenu,
    /// サブメニュー内に配置されるメニュー項目と加工モードのペア
    pub items: Vec<(CheckMenuItem, RefineMode)>,
    /// このグループが属するカテゴリ
    pub category: RefineCategory,
}

/// 変換モードメニュー全体の構成を管理する構造体
pub struct RefineMenu {
    /// 「変換モード」のルートとなるサブメニュー
    pub main_submenu: Submenu,
    /// サブメニューに入れず、ルート直下に表示されるモード項目
    pub normal_items: Vec<(CheckMenuItem, RefineMode)>,
    /// カテゴリごとに整理されたサブメニューのリスト
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

/// 監視方式（ポーリング/イベント）を選択するメニューの構成
pub struct MonitorMenu {
    /// 「監視方式」サブメニュー
    pub main_submenu: Submenu,
    /// 監視方式ごとのチェックメニュー項目
    pub items: Vec<(CheckMenuItem, MonitorMode)>,
}

/// 監視間隔（周期）を選択するメニューの構成
pub struct IntervalMenu {
    /// 「監視周期」サブメニュー
    pub main_submenu: Submenu,
    /// 各時間間隔ごとのチェックメニュー項目
    pub items: Vec<(CheckMenuItem, u64)>,
}

/// クリップボード履歴に関連するメニューの構成
pub struct HistoryMenu {
    /// 「履歴」サブメニュー
    pub main_submenu: Submenu,
    /// 履歴機能の有効/無効を切り替える項目
    pub enabled_item: CheckMenuItem,
    /// 履歴を全削除する項目
    pub clear_item: MenuItem,
    /// 過去のテキスト項目（表示用のIDと実データ）のリスト。メニュー更新時に利用される
    pub records: Mutex<Vec<(tray_icon::menu::MenuId, String)>>,
}

/// 通知設定に関連するメニューの構成
pub struct NotificationMenu {
    /// 「通知」サブメニュー
    pub main_submenu: Submenu,
    /// 成功通知の有効/無効を切り替える項目
    pub enabled_item: CheckMenuItem,
    /// 通知の内容を詳細設定するためのサブメニュー
    pub content_submenu: Submenu,
    /// モード変更を通知するかどうかの項目
    pub notify_mode_item: CheckMenuItem,
    /// 変換結果を通知するかどうかの項目
    pub notify_result_item: CheckMenuItem,
    /// 一時停止の切替を通知するかどうかの項目
    pub notify_pause_item: CheckMenuItem,
}

/// システムトレイアイコンおよび付随するすべてのメニューを保持・管理する構造体
pub struct TrayMenu {
    /// トレイアイコンのハンドル。この構造体がドロップされるとアイコンも消滅します。
    pub _tray_icon: TrayIcon,
    /// 終了項目
    pub quit_item: MenuItem,
    /// 一時停止項目
    pub pause_item: CheckMenuItem,
    /// 変換モード関連メニュー
    pub refine: RefineMenu,
    /// 監視方式関連メニュー
    pub monitor: MonitorMenu,
    /// 監視周期関連メニュー
    pub interval: IntervalMenu,
    /// クリップボード履歴メニュー
    pub history: HistoryMenu,
    /// 通知設定メニュー
    pub notification: NotificationMenu,
    /// ショートカット一覧表示項目
    pub shortcut_list_item: MenuItem,
}

// ======================================================================
// メニュー全体構築
// ======================================================================
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
        let current_mode = state.get_mode();
        let current_interval = state.interval_ms();
        let current_monitor_mode = state.get_monitor_mode();
        let history_enabled = state.is_history_enabled();
        let show_success_notification = state.is_notification_enabled();

        let refine = Self::build_refine_menu(current_mode)?;
        let monitor = Self::build_monitor_menu(current_monitor_mode)?;
        let interval = Self::build_interval_menu(current_interval, current_monitor_mode)?;
        let history = Self::build_history_menu(history_enabled)?;
        let notification = Self::build_notification_menu(
            show_success_notification,
            state.notify_mode(),
            state.notify_result(),
            state.notify_pause(),
        )?;
        notification
            .content_submenu
            .set_enabled(show_success_notification);

        // その他のメニュー
        let pause_item = CheckMenuItem::new("一時停止", true, state.is_paused(), None);
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
}

// ======================================================================
// 変換モードメニュー
// ======================================================================
impl TrayMenu {
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

        for mode in RefineMode::iter() {
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
}

// ======================================================================
// 監視方式メニュー
// ======================================================================
impl TrayMenu {
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
}

// ======================================================================
// 監視周期メニュー
// ======================================================================
impl TrayMenu {
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
}

// ======================================================================
// 履歴メニュー
// ======================================================================
impl TrayMenu {
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
}

// ======================================================================
// 通知メニュー
// ======================================================================
impl TrayMenu {
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
}

// ======================================================================
// 履歴更新
// ======================================================================
impl TrayMenu {
    /// クリップボード履歴リストの内容を現在の状態に合わせて再構築する
    ///
    /// # Arguments
    /// * `state` - 最新の履歴データを持つアプリケーション状態
    ///
    /// # Returns
    /// * `Result<()>` - 更新に成功した場合は `Ok(())` を返します。
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

// ======================================================================
// アイコン作成
// ======================================================================
/// 埋め込まれたアセットからトレイ用のアイコンを作成する
///
/// # Returns
/// * `Result<Icon>` - 作成されたアイコン。失敗した場合はエラーを返します。
pub fn create_icon() -> Result<Icon> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let decoder = png::Decoder::new(icon_bytes.as_slice());
    let mut reader = decoder
        .read_info()
        .context("アイコンPNGのデコードに失敗しました")?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .context("アイコンPNGフレームの読み取りに失敗しました")?;
    let bytes = &buf[..info.buffer_size()];

    // カラータイプに応じて RGBA8 に変換する
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => anyhow::bail!("サポート外PNGカラータイプ: {:?}", other),
    };

    Icon::from_rgba(rgba, info.width, info.height).context("アイコンデータの作成に失敗しました")
}
