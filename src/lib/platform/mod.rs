//! Platform abstraction.
//!
//! This is the single place in the codebase that branches on the target. Task
//! spawning, file watching and file/asset access are exposed here as portable
//! functions and traits; native and web provide their own implementations and the
//! rest of the codebase stays free of `cfg(target_arch = ...)`.

mod task;
mod watcher;

pub use task::Task;
pub use watcher::Watcher;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    FileWatcher, init_logging, origin, read_bytes, read_optional, read_to_string, spawn,
    spawn_task, write_bytes,
};

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::{
    FileWatcher, init_logging, origin, read_bytes, read_optional, read_to_string, spawn,
    spawn_task, write_bytes,
};
