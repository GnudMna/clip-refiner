//! トレイメニュー「履歴」の構築とクリック処理
//!
//! 履歴データ本体は [`crate::tray::history_store`] が保持する

mod event;
mod menu;

pub(crate) use event::handle_history_event;
