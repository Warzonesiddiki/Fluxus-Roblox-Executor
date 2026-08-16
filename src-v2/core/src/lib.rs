pub mod process;
pub mod sandbox;
pub mod lua_api;
pub mod runtime;
pub mod control_api;

/// # WebSocket X Core Library
///
/// The `wsx-core` crate provides two primary capabilities:
///
/// 1. **Process attachment & injection** (`process::Injector`) — safely find,
///    open, allocate, write, and execute code in a remote Windows process.
/// 2. **Sandboxed Luau execution** (`sandbox::Sandbox`) — a zero-trust Luau
///    runtime that strips dangerous libraries, enforces instruction/memory
///    limits, and collects output safely.

pub use process::*;
pub use sandbox::*;