use anyhow::Result;

use super::types::TrayMenu;
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::state::{AppState, LockExt, MenuSnapshot};

// ======================================================================
// 設定同期
// ======================================================================
impl TrayMenu {
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
        let snap: MenuSnapshot = state.menu_snapshot();

        self.refine
            .favorite_records
            .lock_ignore_poison()
            .iter()
            .chain(self.refine.all_mode_items())
            .for_each(|(item, mode)| item.set_checked(snap.active_modes.contains(mode)));
        self.refresh_category_labels(snap.mode);
        self.refine
            .sync_favorite_actions(snap.mode, &snap.favorite_modes);
        self.refine.sync_mode_labels(&snap.favorite_modes);
        if let Err(err) =
            self.refine
                .rebuild_favorites(&snap.active_modes, &snap.favorite_modes, &snap.hotkeys)
        {
            dispatch::log_menu_operation_error("お気に入りメニューの再構築", err);
        }

        self.history.enabled_item.set_checked(snap.history_enabled);
        self.continuous_record
            .enabled_item
            .set_checked(snap.continuous_record_enabled);

        self.refresh_clips(state)?;
        self.refresh_regex_presets_logged(state);
        self.refresh_history(state)?;
        self.refresh_continuous_record(state)?;
        self.sync_auto_processing_state(state);

        Ok(())
    }

    /// 自動変換・連続記録モードの有効状態に応じて自動加工関連のメニュー表示を同期する
    ///
    /// 連続記録モード中は正規表現プリセットメニューを無効化する。
    /// 変換モードメニューは連続記録モード中も操作でき、選ぶと連続記録モードが無効になる。
    /// 「変換モード」サブメニュー名には現在のモードと自動変換のオン・オフを、
    /// 「連続記録モード」サブメニュー名にはオン・オフを表示し、ツールチップにも同じ表記で並べる。
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    pub fn sync_auto_processing_state(&self, state: &AppState) {
        let (continuous_record_enabled, auto_refine_enabled, pipeline) = state.with_config(|c| {
            (
                c.continuous_record_enabled,
                c.auto_refine_enabled,
                c.effective_pipeline(),
            )
        });

        self.refine
            .auto_refine_item
            .set_checked(auto_refine_enabled);
        self.refine.main_submenu.set_text(refine_menu_title(
            continuous_record_enabled,
            auto_refine_enabled,
            &pipeline,
        ));

        self.regex_presets
            .main_submenu
            .set_enabled(!continuous_record_enabled);

        self.continuous_record
            .main_submenu
            .set_text(continuous_record_menu_title(continuous_record_enabled));

        let tooltip =
            auto_processing_tooltip(continuous_record_enabled, auto_refine_enabled, &pipeline);
        if let Err(err) = self.tray_icon.set_tooltip(Some(tooltip)) {
            dispatch::log_menu_operation_error("トレイツールチップの更新", err);
        }
    }

    /// クリップボードワーカー再開メニューの有効状態を更新する
    ///
    /// # Arguments
    /// * `enabled` - メニュー項目を有効にするかどうか
    pub fn set_clipboard_worker_retry_enabled(&self, enabled: bool) {
        self.retry_clipboard_worker_item.set_enabled(enabled);
    }
}

// ======================================================================
// 状態表示
// ======================================================================
/// サブメニュー名に表示するパイプラインの最大段数 (超過分は段数で示す)
const REFINE_MENU_TITLE_MAX_MODES: usize = 2;

/// 「変換モード」サブメニュー名を組み立てる
///
/// トレイを開いた時点で現在のモードと自動変換の状態が分かるよう、両方を含める
///
/// # Arguments
/// * `continuous_record_enabled` - 連続記録が有効かどうか
/// * `auto_refine_enabled` - コピー時の自動変換が有効かどうか
/// * `pipeline` - 自動変換で適用する変換モード列
///
/// # Returns
/// * `String` - サブメニュー名
fn refine_menu_title(
    continuous_record_enabled: bool,
    auto_refine_enabled: bool,
    pipeline: &[RefineMode],
) -> String {
    let modes = if pipeline.len() > REFINE_MENU_TITLE_MAX_MODES {
        format!(
            "{} ほか {} 段",
            RefineMode::pipeline_label(&pipeline[..REFINE_MENU_TITLE_MAX_MODES]),
            pipeline.len() - REFINE_MENU_TITLE_MAX_MODES
        )
    } else {
        RefineMode::pipeline_label(pipeline)
    };
    // 「連続記録モード」と表記をそろえ、自動変換が実際に動くときだけオンとする
    let status = if auto_refine_enabled && !continuous_record_enabled {
        "オン"
    } else {
        "オフ"
    };
    format!("変換モード: {modes} ({status})")
}

/// 「連続記録モード」サブメニュー名を組み立てる
///
/// 「変換モード」と同じく、トレイを開いた時点で有効かどうかが分かるようにする
///
/// # Arguments
/// * `continuous_record_enabled` - 連続記録モードが有効かどうか
///
/// # Returns
/// * `&'static str` - サブメニュー名
fn continuous_record_menu_title(continuous_record_enabled: bool) -> &'static str {
    if continuous_record_enabled {
        "連続記録モード (オン)"
    } else {
        "連続記録モード (オフ)"
    }
}

// ======================================================================
// ツールチップ
// ======================================================================
/// 自動加工の状態を示すトレイツールチップ文字列を組み立てる
///
/// トレイメニューの「変換モード」「連続記録モード」と同じ表記を 1 行ずつ並べる。
/// パイプラインはメニューと同じく段数を要約するため、ツールチップの長さ上限に収まる
///
/// # Arguments
/// * `continuous_record_enabled` - 連続記録モードが有効かどうか
/// * `auto_refine_enabled` - コピー時の自動変換が有効かどうか
/// * `pipeline` - 自動変換で適用する変換モード列
///
/// # Returns
/// * `String` - ツールチップ文字列
fn auto_processing_tooltip(
    continuous_record_enabled: bool,
    auto_refine_enabled: bool,
    pipeline: &[RefineMode],
) -> String {
    format!(
        "ClipRefiner\n{}\n{}",
        refine_menu_title(continuous_record_enabled, auto_refine_enabled, pipeline),
        continuous_record_menu_title(continuous_record_enabled)
    )
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    /// 変換モードのサブメニュー名に現在のモードと自動変換の状態を表示すること
    #[test]
    fn refine_menu_title_reflects_mode_and_state() {
        let single = [RefineMode::UrlDecode];
        let label = RefineMode::UrlDecode.label();
        assert_eq!(
            refine_menu_title(false, true, &single),
            format!("変換モード: {label} (オン)")
        );
        assert_eq!(
            refine_menu_title(false, false, &single),
            format!("変換モード: {label} (オフ)")
        );
        // 連続記録モード中は自動変換が動かないため、設定より優先してオフを示す
        assert_eq!(
            refine_menu_title(true, true, &single),
            format!("変換モード: {label} (オフ)")
        );
    }

    /// 連続記録モードのサブメニュー名にオン・オフを表示すること
    #[test]
    fn continuous_record_menu_title_reflects_state() {
        assert_eq!(continuous_record_menu_title(true), "連続記録モード (オン)");
        assert_eq!(continuous_record_menu_title(false), "連続記録モード (オフ)");
    }

    /// 長いパイプラインは先頭 2 段と残りの段数で表示すること
    #[test]
    fn refine_menu_title_summarizes_long_pipeline() {
        let pipeline = [
            RefineMode::UrlDecode,
            RefineMode::Trim,
            RefineMode::JsonFormat,
            RefineMode::RemoveEmptyLines,
        ];
        assert_eq!(
            refine_menu_title(false, true, &pipeline),
            format!(
                "変換モード: {} ほか 2 段 (オン)",
                RefineMode::pipeline_label(&pipeline[..2])
            )
        );
    }

    /// ツールチップに変換モード・連続記録モードのオン・オフをメニューと同じ表記で表示すること
    #[test]
    fn auto_processing_tooltip_reflects_state() {
        let pipeline = [RefineMode::UrlDecode, RefineMode::Trim];
        let modes = RefineMode::pipeline_label(&pipeline);
        assert_eq!(
            auto_processing_tooltip(false, false, &pipeline),
            format!("ClipRefiner\n変換モード: {modes} (オフ)\n連続記録モード (オフ)")
        );
        assert_eq!(
            auto_processing_tooltip(false, true, &pipeline),
            format!("ClipRefiner\n変換モード: {modes} (オン)\n連続記録モード (オフ)")
        );
        // 連続記録モード中は自動変換の設定より優先してオフを示す
        assert_eq!(
            auto_processing_tooltip(true, true, &pipeline),
            format!("ClipRefiner\n変換モード: {modes} (オフ)\n連続記録モード (オン)")
        );
    }

    /// 最長のパイプラインでもツールチップが Windows の上限 (127 文字) に収まること
    #[test]
    fn auto_processing_tooltip_fits_windows_limit() {
        let longest = RefineMode::iter()
            .max_by_key(|mode| mode.label().chars().count())
            .expect("変換モードが 1 つ以上ある");
        let pipeline = vec![longest; crate::consts::MAX_PIPELINE_LENGTH];
        let tooltip = auto_processing_tooltip(false, true, &pipeline);
        assert!(tooltip.encode_utf16().count() <= 127, "{tooltip}");
    }
}
