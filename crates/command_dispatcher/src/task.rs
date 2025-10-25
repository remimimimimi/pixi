use std::{any::Any, future::Future, pin::Pin, sync::Arc};

/// A boxed future returned by [`TaskSpec::into_future`].
pub type TaskFuture<O, E> = Pin<Box<dyn Future<Output = Result<O, E>> + Send + 'static>>;

/// An identifier describing why a task was enqueued.
#[derive(Debug, Clone)]
pub struct TaskReason {
    inner: Arc<dyn Any + Send + Sync>,
}

impl TaskReason {
    pub fn new(inner: impl Any + Send + Sync) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.inner.downcast_ref()
    }
}

/// A specification for a unit of work that can be executed by the dispatcher.
pub trait TaskSpec: Send + Sync + 'static {
    type Output: Send + 'static;
    type Error: Send + Sync + 'static;
    type Key: Eq + std::hash::Hash + Clone + Send + Sync + 'static;

    /// A stable identifier used to deduplicate concurrent requests.
    fn key(&self) -> Self::Key;

    /// Optional human-readable reason for diagnostics.
    fn reason(&self) -> Option<TaskReason> {
        None
    }

    /// Convert this task specification into an async operation.
    fn into_future(self) -> TaskFuture<Self::Output, Self::Error>;
}
