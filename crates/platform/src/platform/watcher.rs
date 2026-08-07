/// Watches resources for changes so they can be hot-reloaded.
///
/// Natively this watches files on disk; on the web there is nothing to watch yet
/// (a future implementation might listen on a websocket), so it never reports a
/// change. Code that hot-reloads is written against this trait and stays portable.
pub trait Watcher {
    /// Start watching the given resources (file paths natively).
    fn watch(paths: &[&str]) -> Self;

    /// Whether a watched resource changed since the last call.
    fn changed(&self) -> bool;
}
