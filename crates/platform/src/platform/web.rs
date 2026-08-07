use std::future::Future;

use futures::future::FutureExt;

use super::{Task, Watcher};

/// The page origin, used to fetch tiles from a same-origin (proxied) URL so the
/// browser does not block them with CORS.
pub fn origin() -> Option<String> {
    web_sys::window().and_then(|window| window.location().origin().ok())
}

/// The browser viewport size in CSS pixels. winit reports the canvas as 0x0 until
/// it is sized, so the caller uses this to size the surface at startup.
pub fn viewport_size() -> Option<(u32, u32)> {
    let window = web_sys::window()?;
    let width = window.inner_width().ok()?.as_f64()? as u32;
    let height = window.inner_height().ok()?.as_f64()? as u32;
    Some((width.max(1), height.max(1)))
}

/// Initialize logging: route `log` to the browser console and install a panic
/// hook so Rust panics show up with a readable message.
pub fn init_logging(level: log::Level) {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(level);
}

/// Spawn a fire-and-forget task on the browser event loop.
pub fn spawn<F: Future<Output = ()> + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future);
}

/// Spawn a task on the browser event loop and return a handle to its result.
pub fn spawn_task<F, T>(future: F) -> Task<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let (future, handle) = future.remote_handle();
    wasm_bindgen_futures::spawn_local(future);
    Task::new(handle)
}

/// There is no filesystem to watch on the web, so this never reports a change.
/// A future implementation might listen for reload notifications over a websocket.
pub struct FileWatcher;

impl Watcher for FileWatcher {
    fn watch(_paths: &[&str]) -> Self {
        Self
    }

    fn changed(&self) -> bool {
        false
    }
}

/// The web has no filesystem, so text resources are the ones embedded at build time.
pub fn read_to_string(_path: &str, embedded_default: &'static str) -> String {
    embedded_default.to_string()
}

/// There are no optional local files on the web.
pub fn read_optional(_path: &str) -> Option<String> {
    None
}

/// There is no on-disk cache on the web; the browser caches HTTP responses itself.
pub fn read_bytes(_path: &str) -> Option<Vec<u8>> {
    None
}

pub fn write_bytes(_path: &str, _data: &[u8]) {}
