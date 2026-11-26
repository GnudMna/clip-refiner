use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::window::{Window, WindowBuilder};

use crate::config::get_config_dir;

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

/// 復元時にモニターと重なっているべき最小の幅 (物理ピクセル)
const MIN_VISIBLE_WIDTH: i64 = 100;
/// 復元時にモニターと重なっているべき最小の高さ (物理ピクセル、タイトルバーを掴める高さ)
const MIN_VISIBLE_HEIGHT: i64 = 40;

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
        .map_or_default(SettingsWindowGeometry::clamped)
}

/// 現在の設定ウィンドウ幾何を保存する
///
/// 最小化中は位置が (-32000, -32000)、サイズが 0 になるため、保存済みの幾何を維持する
pub(crate) fn save_settings_window_geometry(window: &Window) {
    if window.is_minimized() {
        return;
    }
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

/// 保存済み幾何のサイズをクランプして `WindowBuilder` へ適用する
///
/// 位置は生成後に `apply_saved_position` で適用する
pub(crate) fn apply_geometry_to_builder(
    mut builder: WindowBuilder,
    geometry: SettingsWindowGeometry,
) -> WindowBuilder {
    let geometry = geometry.clamped();
    builder = builder.with_inner_size(LogicalSize::new(geometry.width, geometry.height));
    builder
}

/// 生成済みウィンドウへ保存済み位置を適用する
///
/// 位置が未保存の場合や、保存位置がどのモニターにも十分に重ならない場合
/// (モニター構成の変更など) はモニター中央へ配置する
pub(crate) fn apply_saved_position(window: &Window, geometry: SettingsWindowGeometry) {
    if let (Some(x), Some(y)) = (geometry.x, geometry.y) {
        let scale_factor = window.scale_factor();
        let position = LogicalPosition::new(x, y).to_physical::<i32>(scale_factor);
        let size = window.outer_size();
        let window_rect = Rect {
            x: i64::from(position.x),
            y: i64::from(position.y),
            width: i64::from(size.width),
            height: i64::from(size.height),
        };
        let monitor_rects: Vec<Rect> = window
            .available_monitors()
            .map(|monitor| {
                let origin = monitor.position();
                let extent = monitor.size();
                Rect {
                    x: i64::from(origin.x),
                    y: i64::from(origin.y),
                    width: i64::from(extent.width),
                    height: i64::from(extent.height),
                }
            })
            .collect();
        if is_visible_on_any_monitor(window_rect, &monitor_rects) {
            window.set_outer_position(position);
            return;
        }
    }
    crate::tray::selector::position_window_on_monitor(
        window,
        crate::tray::selector::WindowVerticalAlign::Center,
    );
}

/// 物理ピクセル単位の矩形
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

/// ウィンドウがいずれかのモニターと操作可能な広さで重なっているかを返す
fn is_visible_on_any_monitor(window: Rect, monitors: &[Rect]) -> bool {
    monitors.iter().any(|monitor| {
        let overlap_width =
            (window.x + window.width).min(monitor.x + monitor.width) - window.x.max(monitor.x);
        let overlap_height =
            (window.y + window.height).min(monitor.y + monitor.height) - window.y.max(monitor.y);
        overlap_width >= MIN_VISIBLE_WIDTH && overlap_height >= MIN_VISIBLE_HEIGHT
    })
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

    const MONITOR: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    /// モニター内に収まる位置は表示可能と判定すること
    #[test]
    fn window_inside_monitor_is_visible() {
        let window = Rect {
            x: 100,
            y: 100,
            width: 960,
            height: 600,
        };
        assert!(is_visible_on_any_monitor(window, &[MONITOR]));
    }

    /// 最小化中の座標 (-32000, -32000) は表示不可と判定すること
    #[test]
    fn minimized_position_is_not_visible() {
        let window = Rect {
            x: -32000,
            y: -32000,
            width: 960,
            height: 600,
        };
        assert!(!is_visible_on_any_monitor(window, &[MONITOR]));
    }

    /// 外したモニター上の位置は残りのモニターと重ならなければ表示不可と判定すること
    #[test]
    fn window_on_removed_monitor_is_not_visible() {
        let window = Rect {
            x: 2000,
            y: 100,
            width: 960,
            height: 600,
        };
        assert!(!is_visible_on_any_monitor(window, &[MONITOR]));
    }

    /// 一部がはみ出していても十分に重なっていれば表示可能と判定すること
    #[test]
    fn partially_offscreen_window_is_visible_when_overlap_is_large() {
        let window = Rect {
            x: 1500,
            y: -50,
            width: 960,
            height: 600,
        };
        assert!(is_visible_on_any_monitor(window, &[MONITOR]));
    }

    /// 重なりが掴めない程度に小さい場合は表示不可と判定すること
    #[test]
    fn tiny_overlap_is_not_visible() {
        let window = Rect {
            x: 1900,
            y: 100,
            width: 960,
            height: 600,
        };
        assert!(!is_visible_on_any_monitor(window, &[MONITOR]));
    }

    /// 複数モニターのいずれかに収まっていれば表示可能と判定すること
    #[test]
    fn window_on_secondary_monitor_is_visible() {
        let secondary = Rect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let window = Rect {
            x: 2100,
            y: 100,
            width: 960,
            height: 600,
        };
        assert!(is_visible_on_any_monitor(window, &[MONITOR, secondary]));
    }
}
