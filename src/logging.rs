use std::sync::atomic::AtomicBool;

pub static PRINT_DEBUG_MESSAGES: AtomicBool = AtomicBool::new(false);

#[macro_export]
macro_rules! fail {
    ($message: expr$(,$params: expr)*) => {{
        eprintln!(concat!("\x1B[91m", "ERROR: ", $message, "\x1B[0m"), $($params,)*);
        std::process::exit(1);
    }}
}

#[macro_export]
macro_rules! debug {
    ($message: expr$(,$params: expr)*) => { if $crate::logging::PRINT_DEBUG_MESSAGES.load(::std::sync::atomic::Ordering::SeqCst) { println!(concat!("[DEBUG] ", $message), $($params,)*); } }
}
