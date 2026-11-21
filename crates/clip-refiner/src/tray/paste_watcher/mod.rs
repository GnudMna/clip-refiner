//! 連続記録キュー先頭の貼り付け補助
//!
//! `hotkeys.continuous_record_paste` (`Alt+Shift+X`) ではワーカーがキュー先頭を
//! クリップボードへ載せたあと、メインスレッドが `SendInput` で `Ctrl+V` を送る。
//! `Ctrl+V` 自体は捕捉せず、通常の貼り付けは OS / 他ツールに任せる。

mod paste;

pub(crate) use paste::send_continuous_record_ctrl_v;
pub(crate) use paste::try_paste_continuous_record_front;
