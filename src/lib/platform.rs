//! Cross-platform task spawning.
//!
//! Background work (tile loading, collision building, hover queries) is spawned
//! as a task rather than an OS thread so it also works on the web, which has no
//! threads. Natively the task runs on a background executor thread; on the web it
//! is driven cooperatively by the browser event loop via `spawn_local`.

use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    std::thread::spawn(move || pollster::block_on(future));
}

#[cfg(target_arch = "wasm32")]
pub fn spawn<F: Future<Output = ()> + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future);
}
