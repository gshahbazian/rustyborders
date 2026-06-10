use std::sync::OnceLock;

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
            eprintln!("[rustyborders] {}", format_args!($($arg)*));
        }
    };
}
