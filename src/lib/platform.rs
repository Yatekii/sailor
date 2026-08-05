//! Cross-platform task spawning.
//!
//! Background work (tile loading, collision building, hover queries) is spawned
//! as a task rather than an OS thread so it also works on the web, which has no
//! threads. Natively the tasks run on a shared tokio runtime (which also drives
//! reqwest's network I/O); on the web they are driven by the browser event loop.

use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
fn runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::OnceLock;
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build the tokio runtime")
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    runtime().spawn(future);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn<F: Future<Output = ()> + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future);
}
