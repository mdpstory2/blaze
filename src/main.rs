//! Blaze - A blazingly fast, chunk-based version control system
//!
//! This is the main entry point for the Blaze VCS command-line tool.
//! The actual implementation is split across multiple modules for better
//! organization and maintainability.

use std::error::Error;
use std::process;

fn main() {
    // Set up better panic handling
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("🔥 Blaze encountered an unexpected error:");
        if let Some(location) = panic_info.location() {
            eprintln!(
                "  at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
        if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("  {}", message);
        } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("  {}", message);
        }
        eprintln!("  Please report this issue at: https://github.com/blazevcs/blaze/issues");
    }));

    // Run the CLI application
    if let Err(error) = blaze::run() {
        eprintln!("💥 Error: {}", error);

        // Show additional context for certain error types
        let mut source = error.source();
        while let Some(err) = source {
            eprintln!("   Caused by: {}", err);
            source = err.source();
        }

        process::exit(1);
    }
}
