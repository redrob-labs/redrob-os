//! redrob-local-inference library surface (the binary in `main.rs` wires these
//! together). The supervisor keeps the ~1B router model served on loopback so
//! the agent has an offline fallback when Redrob Console is unreachable
//! (design: docs/design/local-inference.md).
pub mod config;
pub mod model;
pub mod server;
