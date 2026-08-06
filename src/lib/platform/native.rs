use std::future::Future;

use futures::future::FutureExt;

use super::{Task, Watcher};

/// Initialize logging: `pretty_env_logger` to stderr, capped at `level`.
pub fn init_logging(level: log::Level) {
    log::set_max_level(level.to_level_filter());
    pretty_env_logger::init();
}

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

/// Spawn a fire-and-forget background task on the tokio runtime.
pub fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    runtime().spawn(future);
}

/// Spawn a background task and return a handle that resolves to its result.
pub fn spawn_task<F, T>(future: F) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (future, handle) = future.remote_handle();
    runtime().spawn(future);
    Task::new(handle)
}

/// Watches files on disk using `notify`.
pub struct FileWatcher {
    rx: std::sync::mpsc::Receiver<Result<notify::Event, notify::Error>>,
    _watcher: notify::RecommendedWatcher,
}

impl Watcher for FileWatcher {
    fn watch(paths: &[&str]) -> Self {
        use notify::{RecursiveMode, Watcher as _};
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .expect("Unable to create the file watcher.");
        for path in paths {
            if let Err(err) = watcher.watch(std::path::Path::new(path), RecursiveMode::Recursive) {
                log::info!("Failed to start watching {path}:\r\n{err}");
            }
        }
        Self {
            rx,
            _watcher: watcher,
        }
    }

    fn changed(&self) -> bool {
        use notify::{EventKind, event::ModifyKind};
        matches!(
            self.rx.try_recv(),
            Ok(Ok(notify::Event {
                kind: EventKind::Modify(ModifyKind::Data(_)),
                ..
            }))
        )
    }
}

/// Read a text resource from disk, falling back to the embedded default if the
/// file is missing.
pub fn read_to_string(path: &str, embedded_default: &'static str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|_| embedded_default.to_string())
}

/// Read an optional text file from disk, returning `None` if it does not exist.
pub fn read_optional(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Read a binary file from disk, returning `None` if it does not exist.
pub fn read_bytes(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Write a binary file to disk, creating parent directories as needed.
pub fn write_bytes(path: &str, data: &[u8]) {
    if let Some(parent) = std::path::Path::new(path).parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        log::error!("Could not create cache directory for {path}: {e}");
        return;
    }
    if let Err(e) = std::fs::write(path, data) {
        log::error!("Could not write {path}: {e}");
    }
}
