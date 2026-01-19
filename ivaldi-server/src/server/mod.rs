//! Server Module
//!
//! Main server loop and request routing

pub mod protocol;

pub use protocol::{handle_initialize, handle_tools_list, handle_tools_call};
