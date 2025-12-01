pub mod cli_utils;
pub mod command_handlers;
pub mod file_io;
pub mod pipeline_ops;

// Re-export public API
pub use command_handlers::handle_command;
