use std::collections::HashSet;
use std::sync::Arc;

use super::outcome::PatchApplyOutcome;
use crate::tray::event::{set_auto_refine_enabled, set_refine_mode};
use crate::tray::menu::TrayMenu;
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 加工パッチ
// ======================================================================
/// 変換モードとパイプラインのパッチを適用する
///
/// 設定の変更だけを反映し、現在のクリップボードは加工しない
pub(super) fn apply_refine_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(enabled) = patch.auto_refine_enabled
        && set_auto_refine_enabled(state, menu, enabled, Some("設定ウィンドウ"))
    {
        *config_dirty = true;
    }

    if let Some(pipeline) = patch.pipeline.clone() {
        state.with_config_mut(|c| {
            c.pipeline = pipeline;
            if !c.pipeline.is_empty() {
                c.mode = c.pipeline[0];
            }
            c.normalize();
        });
        outcome.pipeline_changed = true;
        sync_refine_menu_checks(state, menu);
        menu.sync_auto_processing_state(state);
        *config_dirty = true;
        outcome.bump_monitor = true;
    }

    if let Some(mode) = patch.mode
        && !outcome.pipeline_changed
        && state.with_config(|c| c.mode) != mode
    {
        if state.with_config(|c| c.pipeline.is_empty()) {
            set_refine_mode(state, menu, mode, quick_selector);
            // 切り替え時点のクリップボードは加工せず、次のコピーから対象にする
            outcome.bump_monitor = true;
        } else {
            state.with_config_mut(|c| c.mode = mode);
            menu.refresh_category_labels(mode);
            *config_dirty = true;
        }
    }
}

/// 変換モードメニューのチェック状態を設定内容へ同期する
fn sync_refine_menu_checks(state: &Arc<AppState>, menu: &TrayMenu) {
    let (mode, active_modes) = state.with_config(|config| {
        (
            config.mode,
            config
                .effective_pipeline()
                .into_iter()
                .collect::<HashSet<_>>(),
        )
    });
    menu.refine
        .all_mode_items()
        .for_each(|(item, mode)| item.set_checked(active_modes.contains(mode)));
    menu.refresh_category_labels(mode);
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::refiner::RefineMode;
    use crate::test_helpers::TrayTestContext;

    /// パッチを適用し、結果と設定変更の有無を返す
    fn apply_patch(ctx: &TrayTestContext, patch: &SettingsPatch) -> (PatchApplyOutcome, bool) {
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;
        apply_refine_patch(
            patch,
            ctx.state(),
            ctx.menu(),
            None,
            &mut outcome,
            &mut config_dirty,
        );
        (outcome, config_dirty)
    }

    /// パイプライン非空時のデフォルトモード変更が `mode` のみ更新しパイプラインを保持すること
    #[test]
    fn default_mode_patch_updates_config_when_pipeline_active() {
        let ctx = TrayTestContext::with_config(|c| {
            c.pipeline = vec![RefineMode::Trim, RefineMode::TrimLines];
            c.mode = RefineMode::Trim;
        });
        let patch = SettingsPatch {
            mode: Some(RefineMode::UrlDecode),
            ..SettingsPatch::default()
        };
        let (outcome, config_dirty) = apply_patch(&ctx, &patch);

        assert_eq!(
            ctx.state().with_config(|c| c.pipeline.clone()),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::UrlDecode);
        assert_eq!(
            ctx.state()
                .with_config(crate::config::AppConfig::effective_pipeline),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert!(config_dirty);
        assert!(!outcome.bump_monitor);
        assert!(!outcome.pipeline_changed);
        ctx.assert_no_command();
    }

    /// パイプライン空時のデフォルトモード変更が設定へ反映され、クリップボードは加工しないこと
    ///
    /// 切り替え時点の内容を加工しないよう、監視状態のリセットだけを要求する
    #[test]
    fn default_mode_patch_does_not_process_clipboard_when_pipeline_empty() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            mode: Some(RefineMode::UrlDecode),
            ..SettingsPatch::default()
        };
        let (outcome, _) = apply_patch(&ctx, &patch);

        assert!(ctx.state().with_config(|c| c.pipeline.is_empty()));
        assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::UrlDecode);
        assert!(outcome.bump_monitor);
        ctx.assert_no_command();
    }

    /// パイプライン変更が設定とメニューへ反映されるようにすること
    #[test]
    fn pipeline_patch_updates_config_and_menu() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            pipeline: Some(vec![RefineMode::Trim, RefineMode::TrimLines]),
            ..SettingsPatch::default()
        };
        let (outcome, config_dirty) = apply_patch(&ctx, &patch);

        assert_eq!(
            ctx.state().with_config(|c| c.pipeline.clone()),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::Trim);
        assert!(config_dirty);
        assert!(outcome.pipeline_changed);
    }
}
