//! システムトレイのメニュー構造体と構築ロジックを提供するモジュール
//!
//! 変換モード、監視設定、登録文字列、履歴、通知などのサブメニューを組み立てる

use std::sync::Mutex;

use super::dispatch;
use super::state::AppState;
use crate::autostart;
use crate::config::MonitorMode;
use crate::refiner::{RefineCategory, RefineMode};
use crate::tray::state::LockExt;

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
mod texts;

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
    /// お気に入り変換モード用サブメニュー
    pub favorites_submenu: Submenu,
    /// お気に入りサブメニュー内の動的項目
    pub favorite_records: Mutex<Vec<(CheckMenuItem, RefineMode)>>,
    /// 現在のモードをお気に入りへ登録する項目
    pub add_favorite_item: MenuItem,
    /// 現在のモードをお気に入りから解除する項目
    pub remove_favorite_item: MenuItem,
    /// サブメニューに入れず、ルート直下に表示されるモード項目
    pub normal_items: Vec<(CheckMenuItem, RefineMode)>,
    /// カテゴリごとに整理されたサブメニューのリスト
    pub groups: Vec<CategoryGroup>,
}

impl RefineMenu {
    /// すべてのモードアイテム(CheckMenuItem と `RefineMode` のペア)を平坦化したイテレータとして返す
    ///
    /// # Returns
    /// すべての `(CheckMenuItem, RefineMode)` ペアを巡回するイテレータ
    pub fn all_mode_items(&self) -> impl Iterator<Item = &(CheckMenuItem, RefineMode)> {
        self.normal_items
            .iter()
            .chain(self.groups.iter().flat_map(|g| g.items.iter()))
    }
}

/// 監視方式(ポーリング/イベント)を選択するメニューの構成
pub struct MonitorMenu {
    /// 「監視方式」サブメニュー
    pub main_submenu: Submenu,
    /// 監視方式ごとのチェックメニュー項目
    pub items: Vec<(CheckMenuItem, MonitorMode)>,
}

/// 監視間隔(周期)を選択するメニューの構成
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
    /// 過去のテキスト項目(表示用の ID とストア内インデックス)のリスト。メニュー更新時に利用される
    pub records: Mutex<Vec<(tray_icon::menu::MenuId, usize)>>,
}

/// 登録文字列メニューの構成
pub struct TextsMenu {
    /// 「登録文字列」サブメニュー
    pub main_submenu: Submenu,
    /// 登録文字列項目 (表示用 ID と設定内インデックス) のリスト
    pub records: Mutex<Vec<(tray_icon::menu::MenuId, usize)>>,
    /// 未登録時に表示するプレースホルダ項目
    pub empty_hint_item: MenuItem,
    /// クリップボードの内容を登録する項目
    pub register_item: MenuItem,
}

/// 通知設定に関連するメニューの構成
pub struct NotificationMenu {
    /// 「通知」サブメニュー
    pub main_submenu: Submenu,
    /// 成功通知の有効/無効を切り替える項目
    pub enabled_item: CheckMenuItem,
    /// モード変更を通知するかどうかの項目
    pub notify_mode_item: CheckMenuItem,
    /// クリップボードの内容を通知に含めるかどうかの項目
    pub notify_result_item: CheckMenuItem,
    /// 一時停止の切替を通知するかどうかの項目
    pub notify_pause_item: CheckMenuItem,
}

/// システムトレイアイコンおよび付随するすべてのメニューを保持・管理する構造体
pub struct TrayMenu {
    /// トレイアイコンのハンドル。この構造体がドロップされるとアイコンも消滅する。
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
    /// 登録文字列メニュー
    pub texts: TextsMenu,
    /// 通知設定メニュー
    pub notification: NotificationMenu,
    /// 設定ファイルを開く項目
    pub open_config_item: MenuItem,
    /// 設定ファイルを再読み込みする項目
    pub reload_config_item: MenuItem,
    /// ショートカット一覧表示項目
    pub shortcut_list_item: MenuItem,
    /// ログイン時の自動起動を切り替える項目
    pub launch_at_login_item: CheckMenuItem,
}

// ======================================================================
// メニュー全体構築
// ======================================================================
impl TrayMenu {
    /// トレイアイコンのメニューを構築する
    ///
    /// 現在のアプリケーション状態に基づいて、各種メニュー項目(変換モード、監視方式、監視周期など)を作成し、
    /// トレイアイコンに設定する
    ///
    /// # Arguments
    /// * `state` - 現在のアプリケーション状態。メニューの初期状態の決定に使用される。
    ///
    /// # Returns
    /// メニューの構築に成功した場合は`Ok(Self)`、失敗した場合は`Err`を返す
    pub fn build(state: &AppState) -> Result<Self> {
        let config = state.with_config(std::clone::Clone::clone);

        let refine = Self::build_refine_menu(config.mode, &config.favorite_modes, &config.hotkeys)?;
        let monitor = Self::build_monitor_menu(config.monitor_mode)?;
        let interval = Self::build_interval_menu(config.interval_ms, config.monitor_mode)?;
        let history = Self::build_history_menu(config.history_enabled)?;
        let texts = Self::build_texts_menu(state)?;
        let notification = Self::build_notification_menu(
            config.notification_settings.enabled,
            config.notification_settings.notify_mode,
            config.notification_settings.notify_result,
            config.notification_settings.notify_pause,
        )?;

        // その他のメニュー
        let pause_item = CheckMenuItem::new("一時停止", true, config.is_paused, None);
        let launch_at_login_item =
            CheckMenuItem::new("ログイン時に起動", true, autostart::is_enabled(), None);
        let open_config_item = MenuItem::new("設定を開く", true, None);
        let reload_config_item = MenuItem::new("設定を再読み込み", true, None);
        let shortcut_list_item = MenuItem::new("ショートカット一覧", true, None);
        let quit_item = MenuItem::new("終了", true, None);

        // メインメニューの組み立て
        // クリップボード加工: 変換モード / 監視 / 登録文字列
        // 補助機能: 履歴 / 通知
        let tray_menu = Menu::new();
        tray_menu
            .append_items(&[
                &refine.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &monitor.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &interval.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &texts.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &history.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &notification.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &open_config_item as &dyn tray_icon::menu::IsMenuItem,
                &reload_config_item as &dyn tray_icon::menu::IsMenuItem,
                &shortcut_list_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &launch_at_login_item as &dyn tray_icon::menu::IsMenuItem,
                &pause_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &quit_item as &dyn tray_icon::menu::IsMenuItem,
            ])
            .context("メニューの組み立てに失敗しました")?;

        // アイコン設定
        let icon = create_icon().context("トレイアイコンの読み込みに失敗しました")?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("ClipRefiner")
            .with_icon(icon)
            .build()
            .context("トレイアイコンのビルドに失敗しました")?;

        let this = Self {
            _tray_icon: tray_icon,
            quit_item,
            pause_item,
            refine,
            monitor,
            interval,
            history,
            texts,
            notification,
            open_config_item,
            reload_config_item,
            shortcut_list_item,
            launch_at_login_item,
        };

        // カテゴリラベルの初期更新
        this.refresh_category_labels(config.mode);

        Ok(this)
    }

    /// 現在の設定内容をメニュー表示へ反映する
    ///
    /// ディスクからの再読み込み後に、チェック状態やサブメニューを同期する
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `Result<()>` - 同期成功時は `Ok(())`、失敗時は `Err`
    pub fn sync_from_config(&self, state: &AppState) -> Result<()> {
        let config = state.with_config(std::clone::Clone::clone);

        self.refine
            .favorite_records
            .lock_ignore_poison()
            .iter()
            .chain(self.refine.all_mode_items())
            .for_each(|(item, mode)| item.set_checked(*mode == config.mode));
        self.refresh_category_labels(config.mode);
        self.refine
            .sync_favorite_actions(config.mode, &config.favorite_modes);
        self.refine.sync_mode_labels(&config.favorite_modes);
        if let Err(err) =
            self.refine
                .rebuild_favorites(config.mode, &config.favorite_modes, &config.hotkeys)
        {
            dispatch::log_menu_operation_error("お気に入りメニューの再構築", err);
        }

        for (item, monitor_mode) in &self.monitor.items {
            item.set_checked(*monitor_mode == config.monitor_mode);
        }
        super::clipboard_monitor::update_monitor_mode_impl(self, config.monitor_mode);

        for (item, ms) in &self.interval.items {
            item.set_checked(*ms == config.interval_ms);
        }

        self.history
            .enabled_item
            .set_checked(config.history_enabled);
        self.notification
            .enabled_item
            .set_checked(config.notification_settings.enabled);
        self.notification
            .notify_mode_item
            .set_checked(config.notification_settings.notify_mode);
        self.notification
            .notify_result_item
            .set_checked(config.notification_settings.notify_result);
        self.notification
            .notify_pause_item
            .set_checked(config.notification_settings.notify_pause);
        self.pause_item.set_checked(config.is_paused);

        self.refresh_texts(state)?;
        self.refresh_history(state)?;

        Ok(())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::{HotkeySettings, MonitorMode};
    use crate::refiner::{RefineCategory, RefineMode};

    use strum::IntoEnumIterator;

    /// 変換モードメニューに全モードが含まれること
    #[test]
    fn build_refine_menu_contains_all_modes() {
        let refine = TrayMenu::build_refine_menu(RefineMode::Trim, &[], &HotkeySettings::default())
            .expect("変換モードメニューの構築に失敗");
        let modes: Vec<_> = refine.all_mode_items().map(|(_, mode)| *mode).collect();
        assert_eq!(modes.len(), RefineMode::iter().count());
        assert!(
            refine
                .all_mode_items()
                .any(|(item, mode)| *mode == RefineMode::Trim && item.is_checked())
        );
    }

    /// 選択カテゴリのサブメニューにチェックプレフィックスが付くこと
    #[test]
    fn refresh_category_labels_marks_current_category() {
        let refine = TrayMenu::build_refine_menu(
            RefineMode::UrlEncode,
            &[RefineMode::UrlEncode],
            &HotkeySettings::default(),
        )
        .expect("変換モードメニューの構築に失敗");
        let tray = TrayMenu {
            _tray_icon: TrayIconBuilder::new()
                .with_icon(create_icon().expect("アイコンの作成に失敗"))
                .build()
                .expect("トレイアイコンのビルドに失敗"),
            quit_item: MenuItem::new("終了", true, None),
            pause_item: CheckMenuItem::new("一時停止", true, false, None),
            refine,
            monitor: TrayMenu::build_monitor_menu(MonitorMode::Polling)
                .expect("監視方式メニューの構築に失敗"),
            interval: TrayMenu::build_interval_menu(1000, MonitorMode::Polling)
                .expect("監視周期メニューの構築に失敗"),
            history: TrayMenu::build_history_menu(false).expect("履歴メニューの構築に失敗"),
            texts: TextsMenu {
                main_submenu: Submenu::new("登録文字列", true),
                records: Mutex::new(Vec::new()),
                empty_hint_item: MenuItem::new("(未登録)", false, None),
                register_item: MenuItem::new("クリップボードを登録", true, None),
            },
            notification: TrayMenu::build_notification_menu(false, true, true, true)
                .expect("通知メニューの構築に失敗"),
            open_config_item: MenuItem::new("設定を開く", true, None),
            reload_config_item: MenuItem::new("設定を再読み込み", true, None),
            shortcut_list_item: MenuItem::new("ショートカット一覧", true, None),
            launch_at_login_item: CheckMenuItem::new("ログイン時に起動", true, false, None),
        };

        tray.refresh_category_labels(RefineMode::UrlEncode);

        let url_group = tray
            .refine
            .groups
            .iter()
            .find(|g| g.category == RefineCategory::UrlActions)
            .expect("URL カテゴリが存在する");
        assert!(url_group.submenu.text().starts_with('✓'));
    }

    /// イベント監視時は監視周期メニューが無効になること
    #[test]
    fn build_interval_menu_disabled_for_event_mode() {
        let interval = TrayMenu::build_interval_menu(1000, MonitorMode::Event)
            .expect("監視周期メニューの構築に失敗");
        assert!(!interval.main_submenu.is_enabled());
    }

    /// ポーリング監視時は現在の間隔だけがチェックされること
    #[test]
    fn build_interval_menu_checks_current_interval() {
        let interval = TrayMenu::build_interval_menu(2000, MonitorMode::Polling)
            .expect("監視周期メニューの構築に失敗");
        assert!(interval.main_submenu.is_enabled());
        assert!(
            interval
                .items
                .iter()
                .any(|(item, ms)| *ms == 2000 && item.is_checked())
        );
        assert!(
            interval
                .items
                .iter()
                .any(|(item, ms)| *ms == 1000 && !item.is_checked())
        );
    }

    /// 通知メニューの有効状態が初期値を反映すること
    #[test]
    fn build_notification_menu_reflects_settings() {
        let notification = TrayMenu::build_notification_menu(true, false, true, false)
            .expect("通知メニューの構築に失敗");
        assert!(notification.enabled_item.is_checked());
        assert!(!notification.notify_mode_item.is_checked());
        assert!(notification.notify_result_item.is_checked());
        assert!(!notification.notify_pause_item.is_checked());
    }
}
