use std::collections::HashSet;
use std::sync::Arc;

use super::outcome::PatchApplyOutcome;

use crate::tray::event::update_refine;
use crate::tray::menu::TrayMenu;
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;
use crate::tray::worker::ClipboardWorkerHandle;

// ======================================================================
// 加工パッチ
// ======================================================================
/// 変換モードとパイプラインのパッチを適用する
pub(super) fn apply_refine_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
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
        *config_dirty = true;
        outcome.bump_monitor = true;
    }

    if let Some(mode) = patch.mode
        && !outcome.pipeline_changed
        && state.with_config(|c| c.mode) != mode
    {
        if state.with_config(|c| c.pipeline.is_empty()) {
            update_refine(state, menu, clipboard_worker, mode, quick_selector);
            outcome.mode_changed = true;
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
    use crate::test_helpers::{ClipboardCommand, TrayTestContext};

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
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_refine_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            None,
            ctx.worker(),
            &mut outcome,
            &mut config_dirty,
        );

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
        assert!(!outcome.mode_changed);
        assert!(!outcome.pipeline_changed);
        ctx.assert_no_command();
    }

    /// パイプライン空時のデフォルトモード変更が `update_refine` 経由で反映されるようにすること
    #[test]
    fn default_mode_patch_uses_update_refine_when_pipeline_empty() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            mode: Some(RefineMode::UrlDecode),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_refine_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            None,
            ctx.worker(),
            &mut outcome,
            &mut config_dirty,
        );

        assert!(ctx.state().with_config(|c| c.pipeline.is_empty()));
        assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::UrlDecode);
        assert!(outcome.mode_changed);
        match ctx.recv_command() {
            ClipboardCommand::ProcessMode(mode) => assert_eq!(mode, RefineMode::UrlDecode),
            other => panic!("ProcessMode コマンドが送信されること: {other:?}"),
        }
    }

    /// パイプライン変更が設定とメニューへ反映されるようにすること
    #[test]
    fn pipeline_patch_updates_config_and_menu() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            pipeline: Some(vec![RefineMode::Trim, RefineMode::TrimLines]),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_refine_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            None,
            ctx.worker(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.pipeline.clone()),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::Trim);
        assert!(config_dirty);
        assert!(outcome.pipeline_changed);
    }
}
