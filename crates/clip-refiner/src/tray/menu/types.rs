use std::sync::Mutex;

use crate::config::MonitorMode;
use crate::refiner::{RefineCategory, RefineMode};

use tray_icon::{
    TrayIcon,
    menu::{CheckMenuItem, MenuItem, Submenu},
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
    /// すべてのモードアイテム (`CheckMenuItem` と `RefineMode` のペア) を平坦化したイテレータとして返す
    ///
    /// # Arguments
    /// * `self` - この `RefineMenu`
    ///
    /// # Returns
    /// * `impl Iterator<Item = &(CheckMenuItem, RefineMode)>` - すべてのモード項目イテレータ
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

/// 連続記録に関連するメニューの構成
pub struct ContinuousRecordMenu {
    /// 「連続記録」サブメニュー
    pub main_submenu: Submenu,
    /// 連続記録の有効/無効を切り替える項目
    pub enabled_item: CheckMenuItem,
    /// 記録を全削除する項目
    pub clear_item: MenuItem,
    /// 記録項目 (表示用 ID とストア内インデックス) のリスト
    pub records: Mutex<Vec<(tray_icon::menu::MenuId, usize)>>,
}

/// 登録クリップメニューの構成
pub struct ClipsMenu {
    /// 「登録クリップ」サブメニュー
    pub main_submenu: Submenu,
    /// 登録クリップ項目 (表示用 ID と設定内インデックス) のリスト
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
    /// デスクトップ通知の有効/無効を切り替える項目
    pub enabled_item: CheckMenuItem,
    /// 加工完了を通知するかどうかの項目
    pub notify_process_item: CheckMenuItem,
    /// 一時停止の切替を通知するかどうかの項目
    pub notify_pause_item: CheckMenuItem,
    /// 履歴復元を通知するかどうかの項目
    pub notify_history_restore_item: CheckMenuItem,
    /// OCR を通知するかどうかの項目
    pub notify_ocr_item: CheckMenuItem,
    /// 登録クリップを通知するかどうかの項目
    pub notify_registered_clip_item: CheckMenuItem,
    /// お気に入りを通知するかどうかの項目
    pub notify_favorites_item: CheckMenuItem,
    /// 連続記録を通知するかどうかの項目
    pub notify_continuous_record_item: CheckMenuItem,
    /// クリップボードの内容を通知に含めるかどうかの項目
    pub notify_result_item: CheckMenuItem,
}

/// システムトレイアイコンおよび付随するすべてのメニューを保持・管理する構造体
pub struct TrayMenu {
    /// トレイアイコンのハンドル。この構造体がドロップされるとアイコンも消滅する。
    pub(in crate::tray::menu) tray_icon: TrayIcon,
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
    /// 連続記録メニュー
    pub continuous_record: ContinuousRecordMenu,
    /// 登録クリップメニュー
    pub clips: ClipsMenu,
    /// 通知設定メニュー
    pub notification: NotificationMenu,
    /// 設定ウィンドウを開く項目
    pub open_settings_window_item: MenuItem,
    /// 停止したクリップボードワーカーを再起動する項目
    pub retry_clipboard_worker_item: MenuItem,
}
