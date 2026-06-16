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
            linux: LinuxWatcher::new().ok(),
        }
    }

    /// イベント監視が利用可能かどうか
    pub fn is_supported(&self) -> bool {
        #[cfg(windows)]
        {
            true
        }
        #[cfg(target_os = "macos")]
        {
            true
        }
        #[cfg(target_os = "linux")]
        {
            self.linux.is_some()
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            false
        }
    }

    /// クリップボード変更を表すトークンを取得する
    ///
    /// 本文を読み取らずに呼び出せる軽量な値です。変更がない間は同じ値を返します。
    pub fn token(&self) -> Option<u64> {
        #[cfg(windows)]
        {
            use clipboard_win::raw::seq_num;
            seq_num().map(|s| s.get() as u64)
        }
        #[cfg(target_os = "macos")]
        {
            macos_change_count()
        }
        #[cfg(target_os = "linux")]
        {
            self.linux.as_ref()?.token()
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }
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
// Linux (X11)
// ======================================================================
#[cfg(target_os = "linux")]
struct LinuxWatcher {
    conn: x11rb::rust_connection::RustConnection,
    clipboard_atom: u32,
}

#[cfg(target_os = "linux")]
impl LinuxWatcher {
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
