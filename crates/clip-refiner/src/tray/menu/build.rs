use super::icon::create_icon;
use super::types::TrayMenu;
use crate::tray::state::{AppState, MenuSnapshot};

use anyhow::{Context, Result};
use tray_icon::{
    TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
};

// ======================================================================
// メニュー全体構築
// ======================================================================
impl TrayMenu {
    /// トレイアイコンのメニューを構築する
    ///
    /// 現在のアプリケーション状態に基づいて、各種メニュー項目 (変換モード・登録クリップ・履歴など) を作成し、
    /// トレイアイコンに設定する。監視方式・通知・更新確認などの設定項目は設定ウィンドウにだけ置く
    ///
    /// # Arguments
    /// * `state` - 現在のアプリケーション状態。メニューの初期状態の決定に使用される。
    ///
    /// # Returns
    /// * `Result<Self>` - メニューの構築に成功した場合は `Ok(Self)`、失敗した場合は `Err`
    pub fn build(state: &AppState) -> Result<Self> {
        let snap: MenuSnapshot = state.menu_snapshot();

        let refine = Self::build_refine_menu(
            snap.mode,
            &snap.active_modes,
            &snap.favorite_modes,
            &snap.hotkeys,
        )?;
        let history = Self::build_history_menu(snap.history_enabled)?;
        let continuous_record = Self::build_continuous_record_menu(snap.continuous_record_enabled)?;
        let clips = Self::build_clips_menu(state)?;
        let regex_presets =
            Self::build_regex_preset_menu(&snap.regex_active_preset_id, &snap.regex_presets)?;
        // 正規表現プリセットは変換モード (`regex-replace` など) の補助設定のため、変換モード内に置く
        refine
            .main_submenu
            .append_items(&[
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &regex_presets.main_submenu,
            ])
            .context("正規表現プリセットメニューの追加に失敗しました")?;

        // その他のメニュー
        let pause_item = CheckMenuItem::new("一時停止", true, snap.is_paused, None);
        let quick_selector_item = MenuItem::new("クイック変換…", true, None);
        let open_settings_window_item = MenuItem::new("設定を開く", true, None);
        let retry_clipboard_worker_item = MenuItem::new("クリップボード監視を再開", true, None);
        retry_clipboard_worker_item.set_enabled(false);
        let quit_item = MenuItem::new("終了", true, None);

        // メインメニューの組み立て
        // 変換: 変換モード (自動変換・正規表現プリセットを含む) / クイック変換
        // クリップボード: 登録クリップ / 履歴 / 連続記録
        // 設定: 設定ウィンドウ
        // 操作: 監視再開 / 一時停止 / 終了
        let tray_menu = Menu::new();
        tray_menu
            .append_items(&[
                &refine.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &quick_selector_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &clips.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &history.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &continuous_record.main_submenu as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &open_settings_window_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &retry_clipboard_worker_item as &dyn tray_icon::menu::IsMenuItem,
                &pause_item as &dyn tray_icon::menu::IsMenuItem,
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
            tray_icon,
            quit_item,
            pause_item,
            refine,
            history,
            continuous_record,
            clips,
            regex_presets,
            quick_selector_item,
            open_settings_window_item,
            retry_clipboard_worker_item,
        };

        // カテゴリラベルの初期更新
        this.refresh_category_labels(snap.mode);
        this.sync_auto_processing_state(state);

        Ok(this)
    }
}
