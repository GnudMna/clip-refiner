mod decision;
mod generation;
mod guard;
mod handle;

pub use generation::{bump_monitor_generation, bump_monitor_generation_unchecked};
pub(crate) use handle::{
    ClipboardObserveResult, ClipboardTrigger, EVENT_POLL_MS, POLL_TICK_MS, handle_clipboard_update,
    observe_continuous_record, recover_continuous_record_on_reset,
};
