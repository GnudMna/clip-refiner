use std::path::PathBuf;

use crate::config::get_config_dir;

use serde::{Deserialize, Serialize};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::window::{Window, WindowBuilder};

// ======================================================================
// 定数
// ======================================================================
const GEOMETRY_FILE: &str = "settings-window.json";

/// 既定の幅 (登録クリップ一覧のプレビュー最大幅と操作ボタンが収まる幅)
const DEFAULT_WIDTH: f64 = 960.0;
const DEFAULT_HEIGHT: f64 = 600.0;
const MIN_WIDTH: f64 = 640.0;
const MIN_HEIGHT: f64 = 480.0;
const MAX_WIDTH: f64 = 3840.0;
const MAX_HEIGHT: f64 = 2160.0;

// ======================================================================
// ウィンドウ幾何
// ======================================================================
/// 設定ウィンドウの論理サイズと位置
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsWindowGeometry {
    /// 内側幅 (論理ピクセル)
    pub width: f64,
    /// 内側高さ (論理ピクセル)
    pub height: f64,
    /// 外枠左上 X (論理ピクセル、未保存時は `None`)
    pub x: Option<f64>,
    /// 外枠左上 Y (論理ピクセル、未保存時は `None`)
    pub y: Option<f64>,
}

impl Default for SettingsWindowGeometry {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            x: None,
            y: None,
        }
    }
}

impl SettingsWindowGeometry {
    /// 許容範囲内にクランプする
    fn clamped(self) -> Self {
        Self {
            width: self.width.clamp(MIN_WIDTH, MAX_WIDTH),
            height: self.height.clamp(MIN_HEIGHT, MAX_HEIGHT),
            x: self.x,
            y: self.y,
        }
    }
}

/// 保存済みの設定ウィンドウ幾何を読み込む
pub(crate) fn load_settings_window_geometry() -> SettingsWindowGeometry {
    let Ok(path) = geometry_file_path() else {
        return SettingsWindowGeometry::default();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return SettingsWindowGeometry::default();
    };
    serde_json::from_str(&contents)
        .ok()
        .map(SettingsWindowGeometry::clamped)
        .unwrap_or_default()
}

/// 現在の設定ウィンドウ幾何を保存する
pub(crate) fn save_settings_window_geometry(window: &Window) {
    let Ok(path) = geometry_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let (x, y) = window
        .outer_position()
        .ok()
        .map_or((None, None), |position| {
            let logical = position.to_logical::<f64>(window.scale_factor());
            (Some(logical.x), Some(logical.y))
        });
    let geometry = SettingsWindowGeometry {
        width: size.width,
        height: size.height,
        x,
        y,
    }
    .clamped();

    if let Ok(json) = serde_json::to_string_pretty(&geometry) {
        let _ = std::fs::write(path, json);
    }
}

/// 保存済み幾何を `WindowBuilder` へ適用する
pub(crate) fn apply_geometry_to_builder(
    mut builder: WindowBuilder,
    geometry: SettingsWindowGeometry,
) -> WindowBuilder {
    let geometry = geometry.clamped();
    builder = builder.with_inner_size(LogicalSize::new(geometry.width, geometry.height));
    builder
}

/// 生成済みウィンドウへ保存済み位置を適用する
pub(crate) fn apply_saved_position(window: &Window, geometry: SettingsWindowGeometry) {
    let Some(x) = geometry.x else {
        crate::tray::selector::position_window_on_monitor(
            window,
            crate::tray::selector::WindowVerticalAlign::Center,
        );
        return;
    };
    let Some(y) = geometry.y else {
        crate::tray::selector::position_window_on_monitor(
            window,
            crate::tray::selector::WindowVerticalAlign::Center,
        );
        return;
    };
    window.set_outer_position(LogicalPosition::new(x, y));
}

/// 設定ウィンドウ幾何ファイルのパスを返す
fn geometry_file_path() -> Result<PathBuf, crate::config::ConfigError> {
    Ok(get_config_dir()?.join(GEOMETRY_FILE))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 範囲外のサイズがクランプされるようにすること
    #[test]
    fn geometry_clamps_size() {
        let geometry = SettingsWindowGeometry {
            width: 10_000.0,
            height: 100.0,
            x: Some(0.0),
            y: Some(0.0),
        }
        .clamped();
        assert!((geometry.width - MAX_WIDTH).abs() < f64::EPSILON);
        assert!((geometry.height - MIN_HEIGHT).abs() < f64::EPSILON);
    }

    /// デフォルト幾何が既定サイズとなるようにすること
    #[test]
    fn default_geometry_matches_initial_window_size() {
        let geometry = SettingsWindowGeometry::default();
        assert!((geometry.width - DEFAULT_WIDTH).abs() < f64::EPSILON);
        assert!((geometry.height - DEFAULT_HEIGHT).abs() < f64::EPSILON);
        assert!(geometry.x.is_none());
        assert!(geometry.y.is_none());
    }

    /// 幾何 JSON を保存して再読込できるようにすること
    #[test]
    fn geometry_round_trips_through_settings_window_json() {
        use crate::config::get_config_dir;
        use crate::test_helpers::with_temp_config_dir;

        with_temp_config_dir(|| {
            let path = get_config_dir()
                .expect("テスト用設定ディレクトリ")
                .join(GEOMETRY_FILE);
            let geometry = SettingsWindowGeometry {
                width: 900.0,
                height: 700.0,
                x: Some(12.0),
                y: Some(34.0),
            };
            let json = serde_json::to_string_pretty(&geometry).expect("幾何 JSON のシリアライズ");
            std::fs::write(&path, json).expect("幾何 JSON の書き込み");

            let loaded = load_settings_window_geometry();
            assert_eq!(loaded, geometry);
        });
    }

    /// 下限未満のサイズがクランプされるようにすること
    #[test]
    fn geometry_clamps_below_minimum_size() {
        let geometry = SettingsWindowGeometry {
            width: 1.0,
            height: 1.0,
            x: None,
            y: None,
        }
        .clamped();
        assert!((geometry.width - MIN_WIDTH).abs() < f64::EPSILON);
        assert!((geometry.height - MIN_HEIGHT).abs() < f64::EPSILON);
    }

    /// ファイル欠落時は既定幾何へフォールバックすること
    #[test]
    fn load_geometry_falls_back_when_file_missing() {
        use crate::test_helpers::with_temp_config_dir;

        with_temp_config_dir(|| {
            let loaded = load_settings_window_geometry();
            assert_eq!(loaded, SettingsWindowGeometry::default());
        });
    }

    /// 不正 JSON は既定幾何へフォールバックすること
    #[test]
    fn load_geometry_falls_back_on_invalid_json() {
        use crate::config::get_config_dir;
        use crate::test_helpers::with_temp_config_dir;

        with_temp_config_dir(|| {
            let path = get_config_dir()
                .expect("テスト用設定ディレクトリ")
                .join(GEOMETRY_FILE);
            std::fs::write(&path, "{not-json").expect("不正 JSON の書き込み");

            let loaded = load_settings_window_geometry();
            assert_eq!(loaded, SettingsWindowGeometry::default());
        });
    }

    /// 読込時に範囲外サイズをクランプすること
    #[test]
    fn load_geometry_clamps_out_of_range_size() {
        use crate::config::get_config_dir;
        use crate::test_helpers::with_temp_config_dir;

        with_temp_config_dir(|| {
            let path = get_config_dir()
                .expect("テスト用設定ディレクトリ")
                .join(GEOMETRY_FILE);
            let geometry = SettingsWindowGeometry {
                width: 10_000.0,
                height: 10.0,
                x: Some(1.0),
                y: Some(2.0),
            };
            let json = serde_json::to_string_pretty(&geometry).expect("幾何 JSON のシリアライズ");
            std::fs::write(&path, json).expect("幾何 JSON の書き込み");

            let loaded = load_settings_window_geometry();
            assert!((loaded.width - MAX_WIDTH).abs() < f64::EPSILON);
            assert!((loaded.height - MIN_HEIGHT).abs() < f64::EPSILON);
            assert_eq!(loaded.x, Some(1.0));
            assert_eq!(loaded.y, Some(2.0));
        });
    }
}
