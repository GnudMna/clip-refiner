// ======================================================================
// パッチ適用結果
// ======================================================================
/// 部分更新パッチの適用結果
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct PatchApplyOutcome {
    pub(super) monitor_mode_changed: bool,
    pub(super) bump_monitor: bool,
    pub(super) mode_changed: bool,
    pub(super) pipeline_changed: bool,
}
