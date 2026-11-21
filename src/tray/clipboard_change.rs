// ======================================================================
// 変更検知ウォッチャー
// ======================================================================
/// クリップボード変更検知用のウォッチャー
pub struct ChangeWatcher {
    #[cfg(target_os = "linux")]
    linux: Option<LinuxWatcher>,
}

impl ChangeWatcher {
    /// プラットフォーム向けのウォッチャーを生成する
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            linux: LinuxWatcher::new(),
        }
    }

    /// イベント監視が利用可能かどうか
    ///
    /// # Returns
    /// * `bool` - 現在のプラットフォームでイベント監視が利用可能な場合は `true`
    pub fn is_supported(&self) -> bool {
        platform_is_supported(self)
    }

    /// クリップボード変更を表すトークンを取得する
    ///
    /// 本文を読み取らずに呼び出せる軽量な値。変更がない間は同じ値を返す。
    pub fn token(&self) -> Option<u64> {
        platform_token(self)
    }
}

// ======================================================================
// プラットフォーム別実装
// ======================================================================
#[cfg(windows)]
fn platform_is_supported(_watcher: &ChangeWatcher) -> bool {
    true
}

#[cfg(target_os = "macos")]
fn platform_is_supported(_watcher: &ChangeWatcher) -> bool {
    true
}

#[cfg(target_os = "linux")]
fn platform_is_supported(watcher: &ChangeWatcher) -> bool {
    watcher.linux.is_some()
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_is_supported(_watcher: &ChangeWatcher) -> bool {
    false
}

#[cfg(windows)]
fn platform_token(_watcher: &ChangeWatcher) -> Option<u64> {
    use clipboard_win::raw::seq_num;

    seq_num().map(|s| u64::from(s.get()))
}

#[cfg(target_os = "macos")]
fn platform_token(_watcher: &ChangeWatcher) -> Option<u64> {
    macos_change_count()
}

#[cfg(target_os = "linux")]
fn platform_token(watcher: &ChangeWatcher) -> Option<u64> {
    watcher.linux.as_ref()?.token()
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_token(_watcher: &ChangeWatcher) -> Option<u64> {
    None
}

// ======================================================================
// macOS
// ======================================================================
#[cfg(target_os = "macos")]
fn macos_change_count() -> Option<u64> {
    use objc2::ClassType;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2_app_kit::NSPasteboard;

    let pasteboard: Option<Retained<NSPasteboard>> =
        unsafe { msg_send![NSPasteboard::class(), generalPasteboard] };

    pasteboard.map(|pb| {
        let count: i64 = unsafe { msg_send![&pb, changeCount] };
        count as u64
    })
}

// ======================================================================
// Linux
// ======================================================================
#[cfg(target_os = "linux")]
enum LinuxWatcher {
    X11(X11Watcher),
    Wayland(WaylandWatcher),
}

#[cfg(target_os = "linux")]
impl LinuxWatcher {
    /// 利用可能なバックエンドでウォッチャーを生成する
    ///
    /// `WAYLAND_DISPLAY` が設定されている場合は Wayland を優先し、
    /// 失敗時は X11 へフォールバックする
    fn new() -> Option<Self> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            && let Some(watcher) = WaylandWatcher::new()
        {
            return Some(Self::Wayland(watcher));
        }

        if let Ok(watcher) = X11Watcher::new() {
            return Some(Self::X11(watcher));
        }

        WaylandWatcher::new().map(Self::Wayland)
    }

    fn token(&self) -> Option<u64> {
        match self {
            Self::X11(watcher) => watcher.token(),
            Self::Wayland(watcher) => watcher.token(),
        }
    }
}

// ======================================================================
// Linux (X11)
// ======================================================================
#[cfg(target_os = "linux")]
struct X11Watcher {
    conn: x11rb::rust_connection::RustConnection,
    clipboard_atom: u32,
}

#[cfg(target_os = "linux")]
impl X11Watcher {
    /// X11 CLIPBOARD 選択オーナーを監視するウォッチャーを生成する
    ///
    /// # Returns
    /// * `Result<Self, x11rb::errors::ConnectionError>` - 接続に成功した場合はウォッチャー、失敗した場合はエラー
    fn new() -> Result<Self, x11rb::errors::ConnectionError> {
        use x11rb::connection::Connection;
        use x11rb::rust_connection::RustConnection;

        let (conn, _screen_num) = RustConnection::connect(None)?;
        let clipboard_atom = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;

        Ok(Self {
            conn,
            clipboard_atom,
        })
    }

    /// 選択オーナーウィンドウ ID をトークンとして返す
    ///
    /// # Returns
    /// * `Option<u64>` - クリップボード選択のオーナーウィンドウ ID。取得失敗時は `None`
    fn token(&self) -> Option<u64> {
        use x11rb::connection::Connection;

        let owner = self
            .conn
            .get_selection_owner(self.clipboard_atom)
            .ok()?
            .reply()
            .ok()?
            .owner;

        Some(owner as u64)
    }
}

// ======================================================================
// Linux (Wayland)
// ======================================================================
#[cfg(target_os = "linux")]
enum WaylandProtocol {
    Ext,
    Wlr,
}

#[cfg(target_os = "linux")]
struct WaylandWatcher {
    token: std::sync::Arc<std::sync::atomic::AtomicU64>,
    _thread: std::thread::JoinHandle<()>,
}

#[cfg(target_os = "linux")]
impl WaylandWatcher {
    /// Wayland data-control プロトコルでクリップボード変更を監視する
    ///
    /// `ext-data-control-v1` を優先し、未対応の compositor では `wlr-data-control` を試す
    fn new() -> Option<Self> {
        use std::sync::Arc;
        use std::sync::atomic::AtomicU64;
        use std::thread;

        let protocol = probe_wayland_protocol()?;
        let token = Arc::new(AtomicU64::new(0));
        let token_for_thread = Arc::clone(&token);

        let thread = thread::spawn(move || run_wayland_listener(token_for_thread, protocol));

        Some(Self {
            token,
            _thread: thread,
        })
    }

    /// 変更回数をトークンとして返す
    fn token(&self) -> Option<u64> {
        use std::sync::atomic::Ordering;

        Some(self.token.load(Ordering::Relaxed))
    }
}

#[cfg(target_os = "linux")]
fn probe_wayland_protocol() -> Option<WaylandProtocol> {
    use wayland_clipboard_listener::{
        WlClipboardPasteStream, WlClipboardPasteStreamWlr, WlListenType,
    };

    if WlClipboardPasteStream::init(WlListenType::ListenOnSelect).is_ok() {
        return Some(WaylandProtocol::Ext);
    }

    if WlClipboardPasteStreamWlr::init(WlListenType::ListenOnSelect).is_ok() {
        return Some(WaylandProtocol::Wlr);
    }

    None
}

#[cfg(target_os = "linux")]
fn run_wayland_listener(
    token: std::sync::Arc<std::sync::atomic::AtomicU64>,
    protocol: WaylandProtocol,
) {
    use wayland_clipboard_listener::{
        WlClipboardPasteStream, WlClipboardPasteStreamWlr, WlListenType,
    };

    match protocol {
        WaylandProtocol::Ext => {
            let Ok(mut stream) = WlClipboardPasteStream::init(WlListenType::ListenOnSelect) else {
                return;
            };
            bump_token_on_clipboard_events(stream.paste_stream(), &token);
        }
        WaylandProtocol::Wlr => {
            let Ok(mut stream) = WlClipboardPasteStreamWlr::init(WlListenType::ListenOnSelect)
            else {
                return;
            };
            bump_token_on_clipboard_events(stream.paste_stream(), &token);
        }
    }
}

#[cfg(target_os = "linux")]
fn bump_token_on_clipboard_events<I>(iter: I, token: &std::sync::Arc<std::sync::atomic::AtomicU64>)
where
    I: Iterator<
        Item = Result<
            wayland_clipboard_listener::ClipBoardListenMessage,
            wayland_clipboard_listener::WlClipboardListenerError,
        >,
    >,
{
    use std::sync::atomic::Ordering;

    for result in iter.flatten() {
        let _ = result;
        token.fetch_add(1, Ordering::Relaxed);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ウォッチャーを生成してトークンを読み取れること
    #[test]
    fn change_watcher_new_and_token() {
        let watcher = ChangeWatcher::new();
        let _ = watcher.token();
    }

    /// 連続呼び出しでトークンが安定すること (変更がなければ同値)
    #[test]
    fn consecutive_tokens_are_stable_when_unchanged() {
        let watcher = ChangeWatcher::new();
        if let (Some(first), Some(second)) = (watcher.token(), watcher.token()) {
            assert_eq!(first, second);
        }
    }

    /// Windows / macOS ではイベント監視がサポートされること
    #[test]
    fn is_supported_on_desktop_platforms() {
        let watcher = ChangeWatcher::new();
        #[cfg(any(windows, target_os = "macos"))]
        assert!(watcher.is_supported());
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = watcher.is_supported();
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        use super::super::bump_token_on_clipboard_events;

        /// 空イテレータではトークンが増えないこと
        #[test]
        fn bump_token_on_empty_iterator() {
            let token = Arc::new(AtomicU64::new(3));
            bump_token_on_clipboard_events(std::iter::empty(), &token);
            assert_eq!(token.load(Ordering::Relaxed), 3);
        }

        /// イベント 1 件ごとにトークンが増えること
        #[test]
        fn bump_token_on_each_event() {
            use wayland_clipboard_listener::ClipBoardListenMessage;

            let token = Arc::new(AtomicU64::new(0));
            let events = vec![
                Ok(ClipBoardListenMessage::OnSelect),
                Ok(ClipBoardListenMessage::OnFinished),
            ];
            bump_token_on_clipboard_events(events.into_iter(), &token);
            assert_eq!(token.load(Ordering::Relaxed), 2);
        }
    }
}
