use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::RemoteHandle;

/// A handle to a spawned task that resolves to its result.
///
/// The task runs on the platform executor; [`Task::try_take`] lets the caller
/// pick up the result once it is ready without blocking and without a side
/// channel. Dropping the task cancels it.
pub struct Task<T> {
    handle: RemoteHandle<T>,
}

impl<T: 'static> Task<T> {
    pub(crate) fn new(handle: RemoteHandle<T>) -> Self {
        Self { handle }
    }

    /// Returns the task's result if it has finished, otherwise `None`.
    ///
    /// The task keeps running while this returns `None`; only call this again on
    /// tasks that have not yet finished.
    pub fn try_take(&mut self) -> Option<T> {
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match Pin::new(&mut self.handle).poll(&mut cx) {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        }
    }
}
