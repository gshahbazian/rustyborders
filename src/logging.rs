use std::sync::OnceLock;
use std::time::Instant;

/// Milliseconds since the first log call. Timing is what distinguishes a
/// transient glitch from a real user action in the event log.
pub fn elapsed_ms() -> u128 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis()
}

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTYBORDERS_LOG").is_ok_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "debug" | "trace"
            )
        })
    })
}

#[macro_export]
macro_rules! rb_log {
    ($($arg:tt)*) => {
        if $crate::logging::enabled() {
            eprintln!(
                "[rustyborders +{:>7}ms] {}",
                $crate::logging::elapsed_ms(),
                format_args!($($arg)*)
            );
        }
    };
}
