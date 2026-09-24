//! Opt-in local startup tracing. Set ZVIM_TRACE_STARTUP=1 to write stages to stderr.
use std::{sync::OnceLock, time::Instant};
static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();
pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("ZVIM_TRACE_STARTUP").is_some())
}
pub fn mark(stage: &str) {
    if enabled() {
        let start = START.get_or_init(Instant::now);
        eprintln!(
            "zvim_startup_us={} stage={stage}",
            start.elapsed().as_micros()
        );
    }
}
