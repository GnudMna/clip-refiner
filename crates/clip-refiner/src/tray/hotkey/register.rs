use super::resolve::ResolvedHotkeys;

use crate::config::HotkeySettings;
use crate::refiner::RefineMode;

use anyhow::Result;
use global_hotkey::{GlobalHotKeyManager, hotkey::HotKey};

// ======================================================================
// お気に入りホットキー割り当て
// ======================================================================
/// お気に入り変換モード用ホットキー割り当て
struct FavoriteHotkeyBinding {
    /// 登録済みホットキー
    hotkey: HotKey,
    /// `favorite_modes` 内のインデックス
    slot_index: usize,
}

// ======================================================================
// ホットキーハンドラ
// ======================================================================
/// グローバルホットキーの登録と管理を行う構造体
///
/// アプリケーションが非アクティブな状態でも、特定のキー入力を監視して
/// モード選択UIの表示や設定の切り替えなどを実行する
pub struct HotkeyHandler {
    pub(super) manager: GlobalHotKeyManager,
    pub(super) quick_selector_hotkey: HotKey,
    pub(super) notification_hotkey: HotKey,
    pub(super) pause_hotkey: HotKey,
    pub(super) quit_hotkey: HotKey,
    pub(super) undo_hotkey: HotKey,
    pub(super) clip_selector_hotkey: HotKey,
    #[cfg(screen_ocr)]
    pub(super) ocr_hotkey: HotKey,
    favorite_hotkeys: Vec<FavoriteHotkeyBinding>,
}

impl HotkeyHandler {
    /// ホットキーハンドラを初期化し、各種ショートカットをシステムに登録する
    ///
    /// # Arguments
    /// * `hotkeys` - 設定ファイルから読み込んだホットキー割り当て
    /// * `favorite_modes` - お気に入り登録済み変換モード
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `HotkeyHandler` インスタンス。登録に失敗した場合はエラーを返す
    pub fn new(hotkeys: &HotkeySettings, favorite_modes: &[RefineMode]) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;
        let resolved = ResolvedHotkeys::from_settings(hotkeys);

        for hotkey in resolved.registered_hotkeys() {
            manager.register(hotkey).map_err(|e| anyhow::anyhow!(e))?;
        }

        let mut handler = Self {
            manager,
            quick_selector_hotkey: resolved.quick_selector,
            notification_hotkey: resolved.notification,
            pause_hotkey: resolved.pause,
            quit_hotkey: resolved.quit,
            undo_hotkey: resolved.undo,
            clip_selector_hotkey: resolved.clip_selector,
            #[cfg(screen_ocr)]
            ocr_hotkey: resolved.ocr,
            favorite_hotkeys: Vec::new(),
        };
        handler.register_favorite_hotkeys(hotkeys, favorite_modes.len())?;
        Ok(handler)
    }

    /// ホットキー割り当てを再登録する
    ///
    /// 設定ファイルの再読み込み後など、再起動なしでショートカットを反映する
    ///
    /// # Arguments
    /// * `hotkeys` - 新しいホットキー割り当て
    /// * `favorite_modes` - お気に入り登録済み変換モード
    ///
    /// # Returns
    /// * `Result<()>` - 再登録成功時は `Ok(())`、失敗時は `Err`
    pub fn reload(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_modes: &[RefineMode],
    ) -> Result<()> {
        for hotkey in self.registered_hotkeys() {
            self.manager
                .unregister(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        self.unregister_favorite_hotkeys()?;

        let resolved = ResolvedHotkeys::from_settings(hotkeys);
        for hotkey in resolved.registered_hotkeys() {
            self.manager
                .register(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        self.quick_selector_hotkey = resolved.quick_selector;
        self.notification_hotkey = resolved.notification;
        self.pause_hotkey = resolved.pause;
        self.quit_hotkey = resolved.quit;
        self.undo_hotkey = resolved.undo;
        self.clip_selector_hotkey = resolved.clip_selector;
        #[cfg(screen_ocr)]
        {
            self.ocr_hotkey = resolved.ocr;
        }

        self.register_favorite_hotkeys(hotkeys, favorite_modes.len())
    }

    /// お気に入り変換モード用ホットキーのみ再登録する
    pub fn reload_favorite_slots(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_count: usize,
    ) -> Result<()> {
        self.unregister_favorite_hotkeys()?;
        self.register_favorite_hotkeys(hotkeys, favorite_count)
    }

    /// 登録済みホットキーを配列として返す
    pub(super) fn registered_hotkeys(&self) -> Vec<HotKey> {
        let mut hotkeys = vec![
            self.quick_selector_hotkey,
            self.notification_hotkey,
            self.pause_hotkey,
            self.quit_hotkey,
            self.undo_hotkey,
            self.clip_selector_hotkey,
        ];
        #[cfg(screen_ocr)]
        hotkeys.push(self.ocr_hotkey);
        hotkeys
    }

    /// お気に入り変換モード用ホットキーを OS へ登録する
    fn register_favorite_hotkeys(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_count: usize,
    ) -> Result<()> {
        let resolved = super::settings::resolve_favorite_slot_hotkeys(
            hotkeys,
            favorite_count,
            &self.registered_hotkeys(),
        );
        for (slot_index, hotkey) in resolved {
            self.manager
                .register(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
            self.favorite_hotkeys
                .push(FavoriteHotkeyBinding { hotkey, slot_index });
        }
        Ok(())
    }

    /// お気に入り変換モード用ホットキーの登録を解除する
    fn unregister_favorite_hotkeys(&mut self) -> Result<()> {
        for binding in &self.favorite_hotkeys {
            self.manager
                .unregister(binding.hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        self.favorite_hotkeys.clear();
        Ok(())
    }

    /// ホットキー ID に対応するお気に入りスロットインデックスを返す
    pub(super) fn favorite_slot_for_hotkey(&self, hotkey_id: u32) -> Option<usize> {
        self.favorite_hotkeys
            .iter()
            .find(|binding| binding.hotkey.id() == hotkey_id)
            .map(|binding| binding.slot_index)
    }
}

#[cfg(test)]
impl HotkeyHandler {
    /// テスト用: 終了ホットキーの ID を返す
    pub(crate) fn quit_hotkey_id(&self) -> u32 {
        self.quit_hotkey.id()
    }

    /// テスト用: 一時停止ホットキーの ID を返す
    pub(crate) fn pause_hotkey_id(&self) -> u32 {
        self.pause_hotkey.id()
    }

    /// テスト用: 通知切替ホットキーの ID を返す
    pub(crate) fn notification_hotkey_id(&self) -> u32 {
        self.notification_hotkey.id()
    }

    /// テスト用: 取り消しホットキーの ID を返す
    pub(crate) fn undo_hotkey_id(&self) -> u32 {
        self.undo_hotkey.id()
    }

    /// テスト用: お気に入りスロットのホットキー ID を返す
    pub(crate) fn favorite_hotkey_id_at(&self, slot_index: usize) -> Option<u32> {
        self.favorite_hotkeys
            .iter()
            .find(|binding| binding.slot_index == slot_index)
            .map(|binding| binding.hotkey.id())
    }

    /// テスト用: OCR ホットキーの ID を返す
    #[cfg(screen_ocr)]
    pub(crate) fn ocr_hotkey_id(&self) -> u32 {
        self.ocr_hotkey.id()
    }
}
