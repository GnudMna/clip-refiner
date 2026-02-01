use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tao::event_loop::EventLoopProxy;

use crate::config::{AppConfig, MonitorMode};
use crate::refiner::RefineMode;

/// アプリケーション内でのカスタムイベント
#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    /// 履歴メニューの更新要求
    RefreshHistory,
}

/// 履歴の最大保持数
pub const HISTORY_LIMIT: usize = 10;

/// Mutexのポイズニングを無視してロックを取得するための拡張トレイト
pub trait LockExt<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// アプリケーション内で共有されるミュータブルな状態
///
/// Mutexのロックに失敗した場合（ポイズニング）、パニックせずに以前の値を返して
/// アプリケーションの実行を継続する方針をとる。
pub struct AppState {
    /// 現在選択されている加工モード
    pub mode: Mutex<RefineMode>,
    /// 監視が一時停止されているかどうか
    pub paused: AtomicBool,
    /// 監視方式（Polling または Event）
    pub monitor_mode: Mutex<MonitorMode>,
    /// 監視スレッドの世代管理用カウンタ。設定変更時に古いスレッドを破棄するために使用
    pub monitor_generation: AtomicU64,
    /// ポーリング時の監視間隔（ミリ秒）
    pub interval_ms: AtomicU64,
    /// 二重加工を防止するために保持される、最後に加工されたテキスト
    pub last_processed_text: Mutex<String>,
    /// 履歴機能が有効かどうか
    pub history_enabled: AtomicBool,
    /// クリップボード履歴（最大10件）
    pub history: Mutex<Vec<String>>,
    /// イベントループへのプロキシ。別スレッドからUIイベント（例: 履歴更新）を送信するために使用される。
    pub proxy: EventLoopProxy<AppEvent>,
}

impl AppState {
    /// デフォルトの設定を読み込んで新しい状態を生成する
    ///
    /// # Returns
    /// * `Self` - 新しく生成された `AppState` インスタンス。
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> Self {
        let config = AppConfig::load();
        Self {
            mode: Mutex::new(config.mode),
            paused: AtomicBool::new(false),
            monitor_mode: Mutex::new(config.monitor_mode),
            monitor_generation: AtomicU64::new(0),
            interval_ms: AtomicU64::new(config.interval_ms),
            last_processed_text: Mutex::new(String::new()),
            history_enabled: AtomicBool::new(config.history_enabled),
            history: Mutex::new(Vec::new()),
            proxy,
        }
    }

    /// 現在の設定をファイルへ保存する。
    pub fn save_config(&self) {
        let config = AppConfig {
            mode: self.get_mode(),
            interval_ms: self.interval_ms.load(Ordering::Relaxed),
            monitor_mode: self.get_monitor_mode(),
            history_enabled: self.history_enabled.load(Ordering::Relaxed),
        };
        if let Err(e) = config.save() {
            eprintln!("設定の保存に失敗: {}", e);
        }
    }

    /// 現在の `RefineMode` をスレッドセーフに取得する。
    ///
    /// # Returns
    /// * `RefineMode` - 現在設定されている `RefineMode`。
    pub fn get_mode(&self) -> RefineMode {
        *self.mode.lock_ignore_poison()
    }

    /// `RefineMode` をスレッドセーフに設定する。
    ///
    /// # Arguments
    /// * `mode` - 新しく設定する `RefineMode`。
    pub fn set_mode(&self, mode: RefineMode) {
        *self.mode.lock_ignore_poison() = mode;
    }

    /// 現在の `MonitorMode` をスレッドセーフに取得する。
    ///
    /// # Returns
    /// * `MonitorMode` - 現在設定されている `MonitorMode`。
    pub fn get_monitor_mode(&self) -> MonitorMode {
        *self.monitor_mode.lock_ignore_poison()
    }

    /// `MonitorMode` をスレッドセーフに設定する。
    ///
    /// # Arguments
    /// * `mode` - 新しく設定する `MonitorMode`。
    pub fn set_monitor_mode(&self, mode: MonitorMode) {
        *self.monitor_mode.lock_ignore_poison() = mode;
    }

    /// 加工済みの最新テキストをスレッド安全に取得する
    ///
    /// # Returns
    /// * `String` - 最後に加工されたテキストのクローン。
    pub fn get_last_processed_text(&self) -> String {
        self.last_processed_text.lock_ignore_poison().clone()
    }

    /// 加工済みの最新テキストをスレッド安全に更新する
    ///
    /// # Arguments
    /// * `text` - 新しく設定する、加工済みのテキスト。
    pub fn set_last_processed_text(&self, text: String) {
        *self.last_processed_text.lock_ignore_poison() = text;
    }

    /// 履歴を取得する
    pub fn get_history(&self) -> Vec<String> {
        self.history.lock_ignore_poison().clone()
    }

    /// 履歴をクリアする
    pub fn clear_history(&self) {
        self.history.lock_ignore_poison().clear();
    }

    /// 履歴にテキストを追加する。
    /// すでに存在する場合は最上位に移動させ、最大10件を保持する。
    pub fn add_to_history(&self, text: String) {
        if text.trim().is_empty() {
            return;
        }

        let mut history = self.history.lock_ignore_poison();

        // 二重登録防止: すでに存在すれば削除して最上位へ
        if let Some(pos) = history.iter().position(|x| x == &text) {
            history.remove(pos);
        }

        history.insert(0, text);

        // 最大10件
        if history.len() > HISTORY_LIMIT {
            history.truncate(HISTORY_LIMIT);
        }

        let _ = self.proxy.send_event(AppEvent::RefreshHistory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tao::event_loop::EventLoopBuilder;

    #[test]
    fn test_app_state_helpers() {
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
        let state = AppState {
            mode: Mutex::new(RefineMode::Trim),
            paused: AtomicBool::new(false),
            monitor_mode: Mutex::new(MonitorMode::Polling),
            monitor_generation: AtomicU64::new(0),
            interval_ms: AtomicU64::new(1000),
            last_processed_text: Mutex::new(String::new()),
            history_enabled: AtomicBool::new(false),
            history: Mutex::new(Vec::new()),
            proxy: event_loop.create_proxy(),
        };

        assert_eq!(state.get_mode(), RefineMode::Trim);
        state.set_mode(RefineMode::UrlEncode);
        assert_eq!(state.get_mode(), RefineMode::UrlEncode);

        assert_eq!(state.get_last_processed_text(), "");
        state.set_last_processed_text("hello".to_string());
        assert_eq!(state.get_last_processed_text(), "hello");

        assert_eq!(state.get_monitor_mode(), MonitorMode::Polling);

        state.interval_ms.store(2000, Ordering::Relaxed);
        assert_eq!(state.interval_ms.load(Ordering::Relaxed), 2000);

        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }
}
