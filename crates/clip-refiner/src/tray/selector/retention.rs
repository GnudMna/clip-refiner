//! セレクター `WebView` の保持 / 破棄ポリシー

use std::time::{Duration, Instant};

use crate::config::SelectorWebviewRetention;
use crate::consts;

use tao::event_loop::ControlFlow;

// ======================================================================
// 保持対象セレクター
// ======================================================================
/// 保持方針を適用できるセレクターウィンドウ
pub(crate) trait RetainedSelector {
    /// ウィンドウを非表示にする (`WebView` は保持)
    fn hide_visual(&self);
    /// `WebView` を破棄する
    fn dispose_webview(&self);
    /// 表示中かどうか
    fn is_visible(&self) -> bool;
    /// `WebView` が生成済みかどうか
    fn is_created(&self) -> bool;
    /// アイドル破棄用の非表示開始時刻
    fn idle_hidden_at(&self) -> Option<Instant>;
    /// アイドル破棄用の非表示開始時刻を更新する
    fn set_idle_hidden_at(&self, at: Option<Instant>);
}

// ======================================================================
// 非表示時の方針適用
// ======================================================================
/// 保持方針に従ってセレクターを非表示にする
///
/// # Arguments
/// * `selector` - 対象セレクター
/// * `retention` - 適用する保持方針
pub(crate) fn hide_with_retention(
    selector: &impl RetainedSelector,
    retention: SelectorWebviewRetention,
) {
    selector.hide_visual();
    match retention {
        SelectorWebviewRetention::Keep => selector.set_idle_hidden_at(None),
        SelectorWebviewRetention::DisposeOnHide => {
            selector.dispose_webview();
            selector.set_idle_hidden_at(None);
        }
        SelectorWebviewRetention::DisposeAfterIdle => {
            selector.set_idle_hidden_at(Some(Instant::now()));
        }
    }
}

// ======================================================================
// 同期 / アイドル破棄
// ======================================================================
/// 現在の保持方針に合わせてセレクター状態を同期する
///
/// `DisposeOnHide` では非表示のまま残った `WebView` を破棄する
/// `DisposeAfterIdle` では期限到来分を破棄し、非表示生成済みなら期限を補完する
///
/// # Arguments
/// * `quick` - クイックセレクター
/// * `clip` - 登録クリップセレクター
/// * `retention` - 現在の保持方針
/// * `idle_minutes` - アイドル破棄までの分数
pub(crate) fn sync_selector_retention(
    quick: &impl RetainedSelector,
    clip: &impl RetainedSelector,
    retention: SelectorWebviewRetention,
    idle_minutes: u64,
) {
    match retention {
        SelectorWebviewRetention::Keep => {
            quick.set_idle_hidden_at(None);
            clip.set_idle_hidden_at(None);
        }
        SelectorWebviewRetention::DisposeOnHide => {
            dispose_if_hidden(quick);
            dispose_if_hidden(clip);
        }
        SelectorWebviewRetention::DisposeAfterIdle => {
            dispose_idle_if_due(quick, idle_minutes);
            dispose_idle_if_due(clip, idle_minutes);
            ensure_idle_mark_if_hidden(quick);
            ensure_idle_mark_if_hidden(clip);
        }
    }
}

/// アイドル破棄の次の起床時刻を `ControlFlow` へ反映する
///
/// # Arguments
/// * `control_flow` - イベントループ制御
/// * `quick` - クイックセレクター
/// * `clip` - 登録クリップセレクター
/// * `retention` - 現在の保持方針
/// * `idle_minutes` - アイドル破棄までの分数
pub(crate) fn apply_idle_control_flow(
    control_flow: &mut ControlFlow,
    quick: &impl RetainedSelector,
    clip: &impl RetainedSelector,
    retention: SelectorWebviewRetention,
    idle_minutes: u64,
) {
    if matches!(*control_flow, ControlFlow::Exit) {
        return;
    }
    if retention != SelectorWebviewRetention::DisposeAfterIdle {
        return;
    }

    let minutes = idle_minutes.clamp(
        consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES,
        consts::MAX_SELECTOR_WEBVIEW_IDLE_MINUTES,
    );
    let deadline = earliest_idle_deadline([quick.idle_hidden_at(), clip.idle_hidden_at()], minutes);
    if let Some(at) = deadline {
        *control_flow = ControlFlow::WaitUntil(at);
    }
}

/// 非表示かつ生成済みなら `WebView` を破棄する
fn dispose_if_hidden(selector: &impl RetainedSelector) {
    if selector.is_created() && !selector.is_visible() {
        selector.dispose_webview();
    }
    selector.set_idle_hidden_at(None);
}

/// アイドル期限到来なら破棄する
fn dispose_idle_if_due(selector: &impl RetainedSelector, idle_minutes: u64) {
    let Some(hidden_at) = selector.idle_hidden_at() else {
        return;
    };
    if selector.is_visible() || !selector.is_created() {
        selector.set_idle_hidden_at(None);
        return;
    }
    if should_dispose_idle(Instant::now(), hidden_at, idle_minutes) {
        selector.dispose_webview();
        selector.set_idle_hidden_at(None);
    }
}

/// 非表示の生成済みセレクターに期限が無ければ現在時刻を記録する
fn ensure_idle_mark_if_hidden(selector: &impl RetainedSelector) {
    if selector.is_created() && !selector.is_visible() && selector.idle_hidden_at().is_none() {
        selector.set_idle_hidden_at(Some(Instant::now()));
    }
}

/// アイドル破棄すべきか判定する
///
/// # Arguments
/// * `now` - 現在時刻
/// * `hidden_at` - 非表示開始時刻
/// * `idle_minutes` - アイドル破棄までの分数
///
/// # Returns
/// * `bool` - 破棄すべきなら `true`
pub(crate) fn should_dispose_idle(now: Instant, hidden_at: Instant, idle_minutes: u64) -> bool {
    now >= idle_dispose_deadline(hidden_at, idle_minutes)
}

/// アイドル破棄の期限時刻を計算する
///
/// # Arguments
/// * `hidden_at` - 非表示開始時刻
/// * `idle_minutes` - アイドル破棄までの分数
///
/// # Returns
/// * `Instant` - 破棄期限
pub(crate) fn idle_dispose_deadline(hidden_at: Instant, idle_minutes: u64) -> Instant {
    let minutes = idle_minutes.clamp(
        consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES,
        consts::MAX_SELECTOR_WEBVIEW_IDLE_MINUTES,
    );
    hidden_at + Duration::from_secs(minutes.saturating_mul(60))
}

/// 複数セレクターの最も早いアイドル破棄期限を返す
///
/// # Arguments
/// * `hidden_ats` - 各セレクターの非表示開始時刻
/// * `idle_minutes` - アイドル破棄までの分数
///
/// # Returns
/// * `Option<Instant>` - 最も早い期限。対象が無ければ `None`
pub(crate) fn earliest_idle_deadline(
    hidden_ats: [Option<Instant>; 2],
    idle_minutes: u64,
) -> Option<Instant> {
    hidden_ats
        .into_iter()
        .flatten()
        .map(|at| idle_dispose_deadline(at, idle_minutes))
        .min()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// アイドル破棄期限が分数どおりに進むこと
    #[test]
    fn idle_dispose_deadline_adds_minutes() {
        let start = Instant::now();
        let deadline = idle_dispose_deadline(start, 5);
        assert_eq!(deadline.duration_since(start), Duration::from_mins(5));
    }

    /// 範囲外の分数はクランプして期限計算すること
    #[test]
    fn idle_dispose_deadline_clamps_minutes() {
        let start = Instant::now();
        let too_small = idle_dispose_deadline(start, 0);
        assert_eq!(
            too_small.duration_since(start),
            Duration::from_secs(consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES * 60)
        );
        let too_large = idle_dispose_deadline(start, 999_999);
        assert_eq!(
            too_large.duration_since(start),
            Duration::from_secs(consts::MAX_SELECTOR_WEBVIEW_IDLE_MINUTES * 60)
        );
    }

    /// 期限前は破棄せず、期限到達後は破棄すること
    #[test]
    fn should_dispose_idle_respects_deadline() {
        let hidden_at = Instant::now();
        assert!(!should_dispose_idle(hidden_at, hidden_at, 5));
        assert!(should_dispose_idle(
            idle_dispose_deadline(hidden_at, 5),
            hidden_at,
            5
        ));
    }

    /// 複数期限のうち最も早いものを返すこと
    #[test]
    fn earliest_idle_deadline_picks_minimum() {
        let earlier = Instant::now();
        let later = earlier + Duration::from_mins(1);
        let deadline =
            earliest_idle_deadline([Some(later), Some(earlier)], 1).expect("期限があること");
        assert_eq!(deadline, idle_dispose_deadline(earlier, 1));
        assert!(earliest_idle_deadline([None, None], 1).is_none());
    }

    /// 保持方針に応じて非表示 / 破棄 / アイドル開始が切り替わること
    #[test]
    fn hide_with_retention_applies_policy() {
        use std::cell::Cell;

        struct FakeSelector {
            disposed: Cell<bool>,
            hidden: Cell<bool>,
            idle_at: Cell<Option<Instant>>,
            created: Cell<bool>,
            visible: Cell<bool>,
        }

        impl RetainedSelector for FakeSelector {
            fn hide_visual(&self) {
                self.hidden.set(true);
                self.visible.set(false);
            }
            fn dispose_webview(&self) {
                self.disposed.set(true);
                self.created.set(false);
            }
            fn is_visible(&self) -> bool {
                self.visible.get()
            }
            fn is_created(&self) -> bool {
                self.created.get()
            }
            fn idle_hidden_at(&self) -> Option<Instant> {
                self.idle_at.get()
            }
            fn set_idle_hidden_at(&self, at: Option<Instant>) {
                self.idle_at.set(at);
            }
        }

        let keep = FakeSelector {
            disposed: Cell::new(false),
            hidden: Cell::new(false),
            idle_at: Cell::new(None),
            created: Cell::new(true),
            visible: Cell::new(true),
        };
        hide_with_retention(&keep, SelectorWebviewRetention::Keep);
        assert!(keep.hidden.get());
        assert!(!keep.disposed.get());
        assert!(keep.idle_at.get().is_none());

        let on_hide = FakeSelector {
            disposed: Cell::new(false),
            hidden: Cell::new(false),
            idle_at: Cell::new(None),
            created: Cell::new(true),
            visible: Cell::new(true),
        };
        hide_with_retention(&on_hide, SelectorWebviewRetention::DisposeOnHide);
        assert!(on_hide.hidden.get());
        assert!(on_hide.disposed.get());

        let idle = FakeSelector {
            disposed: Cell::new(false),
            hidden: Cell::new(false),
            idle_at: Cell::new(None),
            created: Cell::new(true),
            visible: Cell::new(true),
        };
        hide_with_retention(&idle, SelectorWebviewRetention::DisposeAfterIdle);
        assert!(idle.hidden.get());
        assert!(!idle.disposed.get());
        assert!(idle.idle_at.get().is_some());
    }
}
