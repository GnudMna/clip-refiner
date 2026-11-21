use super::resolve::ResolvedHotkeys;

use crate::config::HotkeySettings;
use crate::platform;
use crate::refiner::RefineMode;

use anyhow::Result;
use global_hotkey::{GlobalHotKeyManager, hotkey::HotKey};

// ======================================================================
// 登録結果
// ======================================================================
/// OS へのホットキー登録失敗
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRegistrationFailure {
    /// 割り当て先の表示名
    pub label: String,
    /// 登録を試みたホットキー文字列
    pub binding: String,
    /// 失敗理由
    pub reason: String,
}

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
    pub(super) continuous_record_hotkey: HotKey,
    pub(super) ocr_hotkey: HotKey,
    favorite_hotkeys: Vec<FavoriteHotkeyBinding>,
}

impl HotkeyHandler {
    /// ホットキーハンドラを初期化し、各種ショートカットをシステムに登録する
    ///
    /// 個別の登録に失敗したホットキーはスキップし、失敗一覧を返す
    ///
    /// # Arguments
    /// * `hotkeys` - 設定ファイルから読み込んだホットキー割り当て
    /// * `favorite_modes` - お気に入り登録済み変換モード
    ///
    /// # Returns
    /// * `Result<(Self, Vec<HotkeyRegistrationFailure>)>` - 初期化結果と登録失敗一覧
    pub fn new(
        hotkeys: &HotkeySettings,
        favorite_modes: &[RefineMode],
    ) -> Result<(Self, Vec<HotkeyRegistrationFailure>)> {
        let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;
        let resolved = ResolvedHotkeys::from_settings(hotkeys);
        let mut failures = register_resolved_hotkeys(&manager, hotkeys, &resolved);

        let mut handler = Self {
            manager,
            quick_selector_hotkey: resolved.quick_selector,
            notification_hotkey: resolved.notification,
            pause_hotkey: resolved.pause,
            quit_hotkey: resolved.quit,
            undo_hotkey: resolved.undo,
            clip_selector_hotkey: resolved.clip_selector,
            continuous_record_hotkey: resolved.continuous_record,
            ocr_hotkey: resolved.ocr,
            favorite_hotkeys: Vec::new(),
        };
        failures.extend(handler.register_favorite_hotkeys(hotkeys, favorite_modes.len()));
        Ok((handler, failures))
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
    /// * `Result<Vec<HotkeyRegistrationFailure>>` - 再登録失敗一覧
    pub fn reload(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_modes: &[RefineMode],
    ) -> Result<Vec<HotkeyRegistrationFailure>> {
        for hotkey in self.registered_hotkeys() {
            self.manager
                .unregister(hotkey)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        self.unregister_favorite_hotkeys()?;

        let resolved = ResolvedHotkeys::from_settings(hotkeys);
        let mut failures = register_resolved_hotkeys(&self.manager, hotkeys, &resolved);

        self.quick_selector_hotkey = resolved.quick_selector;
        self.notification_hotkey = resolved.notification;
        self.pause_hotkey = resolved.pause;
        self.quit_hotkey = resolved.quit;
        self.undo_hotkey = resolved.undo;
        self.clip_selector_hotkey = resolved.clip_selector;
        self.continuous_record_hotkey = resolved.continuous_record;
        self.ocr_hotkey = resolved.ocr;

        failures.extend(self.register_favorite_hotkeys(hotkeys, favorite_modes.len()));
        Ok(failures)
    }

    /// お気に入り変換モード用ホットキーのみ再登録する
    ///
    /// # Arguments
    /// * `hotkeys` - ホットキー割り当て
    /// * `favorite_count` - 再登録するお気に入りスロット数
    ///
    /// # Returns
    /// * `Result<Vec<HotkeyRegistrationFailure>>` - 再登録失敗一覧
    pub fn reload_favorite_slots(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_count: usize,
    ) -> Result<Vec<HotkeyRegistrationFailure>> {
        self.unregister_favorite_hotkeys()?;
        Ok(self.register_favorite_hotkeys(hotkeys, favorite_count))
    }

    /// 登録済みホットキーを配列として返す
    pub(super) fn registered_hotkeys(&self) -> Vec<HotKey> {
        let hotkeys = vec![
            self.quick_selector_hotkey,
            self.notification_hotkey,
            self.pause_hotkey,
            self.quit_hotkey,
            self.undo_hotkey,
            self.clip_selector_hotkey,
            self.continuous_record_hotkey,
            self.ocr_hotkey,
        ];
        hotkeys
    }

    /// お気に入り変換モード用ホットキーを OS へ登録する
    fn register_favorite_hotkeys(
        &mut self,
        hotkeys: &HotkeySettings,
        favorite_count: usize,
    ) -> Vec<HotkeyRegistrationFailure> {
        let resolved = super::settings::resolve_favorite_slot_hotkeys(
            hotkeys,
            favorite_count,
            &self.registered_hotkeys(),
        );
        let mut failures = Vec::new();
        for (slot_index, hotkey) in resolved {
            let binding = hotkeys
                .favorite_slot_binding(slot_index)
                .unwrap_or_default();
            if let Some(failure) = try_register_hotkey(
                &self.manager,
                favorite_slot_label(slot_index),
                &binding,
                hotkey,
            ) {
                failures.push(failure);
                continue;
            }
            self.favorite_hotkeys
                .push(FavoriteHotkeyBinding { hotkey, slot_index });
        }
        failures
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

// ======================================================================
// 登録ヘルパー
// ======================================================================
/// 固定ホットキー割り当てを OS へ登録する
fn register_resolved_hotkeys(
    manager: &GlobalHotKeyManager,
    hotkeys: &HotkeySettings,
    resolved: &ResolvedHotkeys,
) -> Vec<HotkeyRegistrationFailure> {
    let mut failures = Vec::new();
    let entries: [(&str, &str, HotKey); 8] = [
        (
            "quick_selector",
            hotkeys.quick_selector.as_str(),
            resolved.quick_selector,
        ),
        (
            "notification",
            hotkeys.notification.as_str(),
            resolved.notification,
        ),
        ("pause", hotkeys.pause.as_str(), resolved.pause),
        ("quit", hotkeys.quit.as_str(), resolved.quit),
        ("undo", hotkeys.undo.as_str(), resolved.undo),
        (
            "clip_selector",
            hotkeys.clip_selector.as_str(),
            resolved.clip_selector,
        ),
        (
            "continuous_record",
            hotkeys.continuous_record.as_str(),
            resolved.continuous_record,
        ),
        ("ocr", hotkeys.ocr.as_str(), resolved.ocr),
    ];

    for (label, binding, hotkey) in entries {
        if let Some(failure) =
            try_register_hotkey(manager, hotkey_action_label(label), binding, hotkey)
        {
            failures.push(failure);
        }
    }

    failures
}

/// 単一ホットキーの OS 登録を試みる
fn try_register_hotkey(
    manager: &GlobalHotKeyManager,
    label: String,
    binding: &str,
    hotkey: HotKey,
) -> Option<HotkeyRegistrationFailure> {
    manager.register(hotkey).err().map(|error| {
        crate::log_warn!("ホットキー '{label}' ({binding}) の登録に失敗: {error}");
        HotkeyRegistrationFailure {
            label,
            binding: binding.to_string(),
            reason: error.to_string(),
        }
    })
}

/// 固定ホットキー割り当ての表示名を返す
fn hotkey_action_label(key: &str) -> String {
    match key {
        "quick_selector" => "クイックセレクター".to_string(),
        "notification" => "成功通知の切替".to_string(),
        "pause" => "一時停止/再開".to_string(),
        "quit" => "終了".to_string(),
        "undo" => "加工の取り消し".to_string(),
        "clip_selector" => "登録クリップセレクター".to_string(),
        "ocr" => "画面 OCR".to_string(),
        "continuous_record" => "連続記録の切替".to_string(),
        other => other.to_string(),
    }
}

/// お気に入りスロットの表示名を返す
fn favorite_slot_label(slot_index: usize) -> String {
    format!("お気に入り {}", slot_index + 1)
}

/// 登録失敗一覧を通知本文へ整形する
pub(crate) fn format_registration_failures(failures: &[HotkeyRegistrationFailure]) -> String {
    if failures.is_empty() {
        return String::new();
    }
    if failures.len() == 1 {
        let failure = &failures[0];
        return format!(
            "{} ({}) を登録できません: {}",
            failure.label, failure.binding, failure.reason
        );
    }

    let lines: Vec<String> = failures
        .iter()
        .map(|failure| {
            format!(
                "・{} ({}): {}",
                failure.label, failure.binding, failure.reason
            )
        })
        .collect();
    format!(
        "{} 件のホットキーを登録できません:\n{}",
        failures.len(),
        lines.join("\n")
    )
}

/// 登録失敗をデスクトップ通知で表示する
pub(crate) fn notify_registration_failures(failures: &[HotkeyRegistrationFailure]) {
    if failures.is_empty() {
        return;
    }
    let body = truncate_notification_body(&format_registration_failures(failures), 240);
    platform::show_notification("ホットキー", &body);
}

/// 通知本文の最大文字数に合わせて切り詰める
fn truncate_notification_body(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    format!("{}...", body.chars().take(keep).collect::<String>())
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

    /// テスト用: 連続記録切替ホットキーの ID を返す
    pub(crate) fn continuous_record_hotkey_id(&self) -> u32 {
        self.continuous_record_hotkey.id()
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
    pub(crate) fn ocr_hotkey_id(&self) -> u32 {
        self.ocr_hotkey.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 登録失敗メッセージが単一失敗を整形すること
    #[test]
    fn format_registration_failures_single() {
        let message = format_registration_failures(&[HotkeyRegistrationFailure {
            label: "終了".to_string(),
            binding: "Alt+Shift+Q".to_string(),
            reason: "already registered".to_string(),
        }]);
        assert!(message.contains("終了"));
        assert!(message.contains("Alt+Shift+Q"));
        assert!(message.contains("already registered"));
    }

    /// 登録失敗メッセージが複数失敗を整形すること
    #[test]
    fn format_registration_failures_multiple() {
        let message = format_registration_failures(&[
            HotkeyRegistrationFailure {
                label: "終了".to_string(),
                binding: "Alt+Shift+Q".to_string(),
                reason: "a".to_string(),
            },
            HotkeyRegistrationFailure {
                label: "一時停止/再開".to_string(),
                binding: "Alt+Shift+P".to_string(),
                reason: "b".to_string(),
            },
        ]);
        assert!(message.contains("2 件"));
        assert!(message.contains("・終了"));
        assert!(message.contains("・一時停止/再開"));
    }
}
