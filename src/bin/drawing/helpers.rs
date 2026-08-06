use std::borrow::Cow;

use wgpu::naga::ShaderStage;

pub fn load_glsl(code: &str, stage: ShaderStage) -> wgpu::ShaderSource<'_> {
    wgpu::ShaderSource::Glsl {
        shader: Cow::Borrowed(code),
        stage,
        defines: &[],
    }
}

/// Watches the shader files for changes to hot-reload them.
///
/// Natively this uses `notify`; on the web there is no filesystem, so it is a
/// no-op and shaders are the ones embedded at build time.
#[cfg(not(target_arch = "wasm32"))]
pub struct ShaderWatcher {
    rx: std::sync::mpsc::Receiver<Result<notify::Event, notify::Error>>,
    _watcher: notify::RecommendedWatcher,
}

#[cfg(not(target_arch = "wasm32"))]
impl ShaderWatcher {
    pub fn new(paths: &[&str]) -> Self {
        use notify::{RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .expect("Unable to create the shader watcher.");
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

    /// Returns `true` if a watched shader file changed since the last call.
    pub fn changed(&self) -> bool {
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

#[cfg(target_arch = "wasm32")]
pub struct ShaderWatcher;

#[cfg(target_arch = "wasm32")]
impl ShaderWatcher {
    pub fn new(_paths: &[&str]) -> Self {
        Self
    }

    pub fn changed(&self) -> bool {
        false
    }
}
