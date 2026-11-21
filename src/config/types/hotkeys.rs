use crate::consts;
use crate::hotkey_binding::parse_hotkey_binding;
use crate::refiner::RefineMode;

use serde::{Deserialize, Serialize};

// ======================================================================
// ホットキー設定
// ======================================================================
/// グローバルホットキーの割り当て
///
/// 各フィールドは `Alt+Shift+S` 形式の文字列で指定する
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeySettings {
    /// クイックセレクターの表示・非表示
    #[serde(default = "default_hotkey_quick_selector", alias = "selector")]
    pub quick_selector: String,
    /// 成功通知のON/OFF切替
    #[serde(default = "default_hotkey_notification")]
    pub notification: String,
    /// 監視の一時停止・再開
    #[serde(default = "default_hotkey_pause")]
    pub pause: String,
    /// アプリケーションの終了
    #[serde(default = "default_hotkey_quit")]
    pub quit: String,
    /// 直近の加工を取り消す
    #[serde(default = "default_hotkey_undo")]
    pub undo: String,
    /// 登録クリップセレクターの表示・非表示
    #[serde(default = "default_hotkey_clip_selector")]
    pub clip_selector: String,
    /// 画面範囲選択 OCR の開始
    #[serde(default = "default_hotkey_ocr")]
    pub ocr: String,
    /// お気に入り変換モード用ホットキー (登録順インデックスに対応。空文字で無効)
    #[serde(default)]
    pub favorite_mode_slots: Vec<String>,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            quick_selector: default_hotkey_quick_selector(),
            notification: default_hotkey_notification(),
            pause: default_hotkey_pause(),
            quit: default_hotkey_quit(),
            undo: default_hotkey_undo(),
            clip_selector: default_hotkey_clip_selector(),
            ocr: default_hotkey_ocr(),
            favorite_mode_slots: Vec::new(),
        }
    }
}

impl HotkeySettings {
    /// ショートカット一覧表示用の文字列を生成する
    pub fn shortcut_list_text(&self, favorite_modes: &[RefineMode]) -> String {
        let mut lines = vec![
            format!("{}: クイックセレクター", self.quick_selector),
            format!("{}: 登録クリップセレクター", self.clip_selector),
        ];
        if crate::platform::supports_screen_ocr() {
            lines.push(format!("{}: 画面 OCR", self.ocr));
        }
        lines.extend([
            format!("{}: 成功通知の切替", self.notification),
            format!("{}: 一時停止/再開", self.pause),
            format!("{}: 加工の取り消し", self.undo),
            format!("{}: 終了", self.quit),
        ]);
        for (index, mode) in favorite_modes.iter().enumerate() {
            if let Some(binding) = self.favorite_slot_binding(index) {
                lines.push(format!("{binding}: {} (お気に入り)", mode.label()));
            }
        }
        lines.join("\n")
    }

    /// お気に入りスロットのホットキー文字列を返す
    ///
    /// 設定で空文字が指定されたスロットは `None` を返す
    pub fn favorite_slot_binding(&self, index: usize) -> Option<String> {
        if index >= consts::MAX_FAVORITE_MODES {
            return None;
        }
        if let Some(binding) = self.favorite_mode_slots.get(index) {
            if binding.is_empty() {
                return None;
            }
            return Some(binding.clone());
        }
        default_favorite_slot_binding(index)
    }

    /// お気に入りスロット用ホットキーを解決する
    ///
    /// 固定ホットキーおよび先に割り当てたスロットと重複する割り当ては除外する
    pub fn resolve_favorite_slot_hotkeys(
        &self,
        favorite_count: usize,
        reserved: &[global_hotkey::hotkey::HotKey],
    ) -> Vec<(usize, global_hotkey::hotkey::HotKey)> {
        use std::collections::HashSet;

        let mut used_ids: HashSet<u32> = reserved
            .iter()
            .map(global_hotkey::hotkey::HotKey::id)
            .collect();
        let mut resolved = Vec::new();

        for index in 0..favorite_count.min(consts::MAX_FAVORITE_MODES) {
            let Some(binding) = self.favorite_slot_binding(index) else {
                continue;
            };
            let hotkey = match parse_hotkey_binding(&binding) {
                Ok(hotkey) => hotkey,
                Err(e) => {
                    crate::log_warn!(
                        "お気に入りスロット {} のホットキー解析に失敗: {} (指定値: '{}')",
                        index + 1,
                        e,
                        binding
                    );
                    continue;
                }
            };
            let id = hotkey.id();
            if used_ids.contains(&id) {
                crate::log_warn!(
                    "お気に入りスロット {} のホットキー '{}' は他の割り当てと重複するため無効",
                    index + 1,
                    binding
                );
                continue;
            }
            used_ids.insert(id);
            resolved.push((index, hotkey));
        }

        resolved
    }

    /// 不正なホットキー文字列をデフォルト値へ置き換える
    pub fn fix_invalid(&mut self) {
        fix_hotkey_field(
            &mut self.quick_selector,
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            "quick_selector",
        );
        fix_hotkey_field(
            &mut self.notification,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            "notification",
        );
        fix_hotkey_field(&mut self.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause");
        fix_hotkey_field(&mut self.quit, consts::DEFAULT_HOTKEY_QUIT, "quit");
        fix_hotkey_field(&mut self.undo, consts::DEFAULT_HOTKEY_UNDO, "undo");
        fix_hotkey_field(
            &mut self.clip_selector,
            consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
            "clip_selector",
        );
        fix_hotkey_field(&mut self.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr");
        self.normalize_favorite_mode_slots();
    }

    /// お気に入りスロット用ホットキー設定を正規化する
    pub(crate) fn normalize_favorite_mode_slots(&mut self) {
        if self.favorite_mode_slots.len() > consts::MAX_FAVORITE_MODES {
            self.favorite_mode_slots
                .truncate(consts::MAX_FAVORITE_MODES);
        }
        for (index, slot) in self.favorite_mode_slots.iter_mut().enumerate() {
            if slot.is_empty() {
                continue;
            }
            if parse_hotkey_binding(slot).is_err() {
                crate::log_warn!(
                    "お気に入りスロット {} のホットキー設定が無効なため無効化する (指定値: '{slot}')",
                    index + 1
                );
                slot.clear();
            }
        }
    }
}

/// 不正なホットキー文字列をデフォルト値へ置き換える
///
/// # Arguments
/// * `field` - 不正なホットキー文字列
/// * `default` - デフォルトホットキー文字列
/// * `label` - ホットキー設定のラベル
fn fix_hotkey_field(field: &mut String, default: &str, label: &str) {
    if parse_hotkey_binding(field).is_err() {
        crate::log_warn!(
            "ホットキー設定 '{label}' が無効なためデフォルト '{default}' に置き換える (指定値: '{field}')"
        );
        *field = default.to_string();
    }
}

/// クイックセレクターのデフォルトホットキーを返す
fn default_hotkey_quick_selector() -> String {
    consts::DEFAULT_HOTKEY_QUICK_SELECTOR.to_string()
}

/// 成功通知のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 成功通知のデフォルトホットキー
fn default_hotkey_notification() -> String {
    consts::DEFAULT_HOTKEY_NOTIFICATION.to_string()
}

/// 一時停止のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 一時停止のデフォルトホットキー
fn default_hotkey_pause() -> String {
    consts::DEFAULT_HOTKEY_PAUSE.to_string()
}

/// 終了のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 終了のデフォルトホットキー
fn default_hotkey_quit() -> String {
    consts::DEFAULT_HOTKEY_QUIT.to_string()
}

/// 加工取り消しのデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 加工取り消しのデフォルトホットキー
fn default_hotkey_undo() -> String {
    consts::DEFAULT_HOTKEY_UNDO.to_string()
}

/// 登録クリップセレクターのデフォルトホットキーを返す
fn default_hotkey_clip_selector() -> String {
    consts::DEFAULT_HOTKEY_CLIP_SELECTOR.to_string()
}

/// 画面 OCR のデフォルトホットキーを返す
fn default_hotkey_ocr() -> String {
    consts::DEFAULT_HOTKEY_OCR.to_string()
}

/// お気に入りスロットのデフォルトホットキー文字列を返す
fn default_favorite_slot_binding(index: usize) -> Option<String> {
    if index < 9 {
        return Some(format!("Alt+Shift+{}", index + 1));
    }
    if index < consts::MAX_FAVORITE_MODES {
        return Some(format!("Alt+Shift+F{}", index - 8));
    }
    None
}
