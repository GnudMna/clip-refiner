use std::sync::Mutex;

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
    /// サブメニュー内に配置されるメニュー項目と変換モードのペア
    pub items: Vec<(CheckMenuItem, RefineMode)>,
    /// このグループが属するカテゴリ
    pub category: RefineCategory,
}

/// 変換モードメニュー全体の構成を管理する構造体
pub struct RefineMenu {
    /// 「変換モード」のルートとなるサブメニュー
    pub main_submenu: Submenu,
    /// コピー時の自動変換を有効にする項目 (`auto_refine_enabled`)
    pub auto_refine_item: CheckMenuItem,
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
    /// 直近の記録を 1 件取り消す項目
    pub undo_item: MenuItem,
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

/// 正規表現プリセット選択メニューの構成
pub struct RegexPresetMenu {
    /// 「正規表現プリセット」サブメニュー
    pub main_submenu: Submenu,
    /// プリセット項目 (`CheckMenuItem` とプリセット ID)
    pub records: Mutex<Vec<(CheckMenuItem, String)>>,
    /// 未登録時に表示するプレースホルダ項目
    pub empty_hint_item: MenuItem,
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
    /// クリップボード履歴メニュー
    pub history: HistoryMenu,
    /// 連続記録メニュー
    pub continuous_record: ContinuousRecordMenu,
    /// 登録クリップメニュー
    pub clips: ClipsMenu,
    /// 正規表現プリセット選択メニュー
    pub regex_presets: RegexPresetMenu,
    /// クイックセレクターを開く項目
    pub quick_selector_item: MenuItem,
    /// 設定ウィンドウを開く項目
    pub open_settings_window_item: MenuItem,
    /// 停止したクリップボードワーカーを再起動する項目
    pub retry_clipboard_worker_item: MenuItem,
}
