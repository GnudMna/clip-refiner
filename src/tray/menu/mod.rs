use std::sync::Mutex;

use super::state::AppState;
use crate::config::MonitorMode;
use crate::refiner::{RefineCategory, RefineMode};

use anyhow::{Context, Result};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

mod history;
mod icon;
mod monitor;
mod notification;
mod refine;

pub use icon::create_icon;

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
        let config = state.with_config(|c| c.clone());

        let refine = Self::build_refine_menu(config.mode)?;
        let monitor = Self::build_monitor_menu(config.monitor_mode)?;
        let interval = Self::build_interval_menu(config.interval_ms, config.monitor_mode)?;
        let history = Self::build_history_menu(config.history_enabled)?;
        let notification = Self::build_notification_menu(
            config.notification_settings.enabled,
            config.notification_settings.notify_mode,
            config.notification_settings.notify_result,
            config.notification_settings.notify_pause,
        )?;
        notification
            .content_submenu
            .set_enabled(config.notification_settings.enabled);

        // その他のメニュー
        let pause_item = CheckMenuItem::new("一時停止", true, config.is_paused, None);
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
        this.refresh_category_labels(config.mode);

        Ok(this)
    }
}
