mod decision;
mod generation;
mod guard;
mod handle;

pub use generation::bump_monitor_generation;
pub(crate) use guard::ObservedClipboard;
pub(crate) use handle::{
    ClipboardObserveResult, ClipboardTrigger, EVENT_POLL_MS, handle_clipboard_update,
    observe_continuous_record, recover_continuous_record_on_reset,
};
