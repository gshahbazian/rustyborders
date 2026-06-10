pub mod cf;
pub mod geometry;
#[cfg(target_os = "macos")]
pub mod mach;
#[cfg(target_os = "macos")]
pub mod macho;
#[cfg(target_os = "macos")]
pub mod os;
#[cfg(target_os = "macos")]
pub mod skylight;

#[cfg(not(target_os = "macos"))]
compile_error!("rustyborders only supports macOS.");
