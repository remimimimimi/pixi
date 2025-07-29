//! Core task abstractions and traits

use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generate a new unique task ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current state of a task during execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// Task is waiting for dependencies to complete
    Pending,
    /// Task is currently executing
    Running { 
        #[serde(skip, default = "Instant::now")]
        started_at: Instant 
    },
    /// Task completed successfully
    Completed { 
        #[serde(skip, default = "Instant::now")]
        started_at: Instant, 
        #[serde(skip, default = "Instant::now")]
        completed_at: Instant,
        duration: Duration,
    },
    /// Task failed with an error
    Failed { 
        #[serde(skip, default = "Instant::now")]
        started_at: Instant, 
        #[serde(skip, default = "Instant::now")]
        failed_at: Instant,
        duration: Duration,
        error: String,
    },
    /// Task was cancelled before or during execution
    Cancelled,
}

impl TaskState {
    /// Check if the task is in a final state (completed, failed, or cancelled)
    pub fn is_finished(&self) -> bool {
        matches!(self, TaskState::Completed { .. } | TaskState::Failed { .. } | TaskState::Cancelled)
    }

    /// Check if the task is currently running
    pub fn is_running(&self) -> bool {
        matches!(self, TaskState::Running { .. })
    }

    /// Get the duration of the task if it has finished
    pub fn duration(&self) -> Option<Duration> {
        match self {
            TaskState::Completed { duration, .. } | TaskState::Failed { duration, .. } => Some(*duration),
            _ => None,
        }
    }
}

/// Result of executing a task
#[derive(Debug, Clone)]
pub enum TaskResult<T, E> {
    /// Task completed successfully with output
    Success(T),
    /// Task failed with error
    Error(E),
    /// Task was cancelled
    Cancelled,
}

impl<T, E> TaskResult<T, E> {
    /// Convert to a standard Result, treating cancellation as an error
    pub fn into_result(self) -> Result<T, TaskError<E>> {
        match self {
            TaskResult::Success(output) => Ok(output),
            TaskResult::Error(err) => Err(TaskError::Execution(err)),
            TaskResult::Cancelled => Err(TaskError::Cancelled),
        }
    }

    /// Check if the result is successful
    pub fn is_success(&self) -> bool {
        matches!(self, TaskResult::Success(_))
    }

    /// Check if the result is an error
    pub fn is_error(&self) -> bool {
        matches!(self, TaskResult::Error(_))
    }

    /// Check if the result is cancelled
    pub fn is_cancelled(&self) -> bool {
        matches!(self, TaskResult::Cancelled)
    }
}

/// Error types for task execution
#[derive(Debug, Clone, thiserror::Error)]
pub enum TaskError<E> {
    #[error("Task execution failed: {0}")]
    Execution(E),
    #[error("Task was cancelled")]
    Cancelled,
}

impl<E: PartialEq> PartialEq for TaskError<E> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TaskError::Execution(a), TaskError::Execution(b)) => a == b,
            (TaskError::Cancelled, TaskError::Cancelled) => true,
            _ => false,
        }
    }
}

impl<E: Eq> Eq for TaskError<E> {}

/// Core trait that all tasks must implement
///
/// This trait defines the interface for asynchronous tasks that can be executed
/// by the Doge executor. Tasks must be:
/// - Cloneable for deduplication
/// - Hashable for identifying duplicates
/// - Debuggable for logging and diagnostics
#[async_trait::async_trait]
pub trait AsyncTask: Clone + Debug + Hash + Eq + Send + Sync + 'static {
    /// The type of output produced by this task
    type Output: Clone + Send + Sync + 'static;
    
    /// The type of error this task can produce
    type Error: Clone + Debug + Send + Sync + 'static;

    /// Execute the task asynchronously
    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        // Default implementation for backward compatibility
        self.execute_with_context(&crate::context::TaskContext::new()).await
    }

    /// Execute the task asynchronously with access to dependency results
    async fn execute_with_context(&self, _context: &crate::context::TaskContext) -> Result<Self::Output, Self::Error> {
        // Default implementation delegates to the old execute method
        self.execute().await
    }

    /// Get a human-readable name for this task (used for logging and debugging)
    fn name(&self) -> String {
        format!("{:?}", self)
    }

    /// Get the estimated duration for this task (used for scheduling hints)
    fn estimated_duration(&self) -> Option<Duration> {
        None
    }

    /// Get the priority of this task (higher values = higher priority)
    fn priority(&self) -> TaskPriority {
        TaskPriority::Normal
    }

    /// Check if this task can be retried on failure
    fn is_retryable(&self) -> bool {
        true
    }

    /// Get the maximum number of retry attempts for this task
    fn max_retries(&self) -> u32 {
        3
    }

    /// Get the delay between retry attempts
    fn retry_delay(&self) -> Duration {
        Duration::from_millis(100)
    }
}

/// Priority levels for task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Metadata associated with a task during execution
#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub id: TaskId,
    pub name: String,
    pub priority: TaskPriority,
    pub estimated_duration: Option<Duration>,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub attempt: u32,
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
    pub dependents: Vec<TaskId>,
}

impl TaskMetadata {
    /// Create new task metadata
    pub fn new<T: AsyncTask>(task: &T) -> Self {
        Self {
            id: TaskId::new(),
            name: task.name(),
            priority: task.priority(),
            estimated_duration: task.estimated_duration(),
            max_retries: task.max_retries(),
            retry_delay: task.retry_delay(),
            attempt: 0,
            state: TaskState::Pending,
            dependencies: Vec::new(),
            dependents: Vec::new(),
        }
    }

    /// Mark the task as started
    pub fn mark_started(&mut self) {
        self.state = TaskState::Running {
            started_at: Instant::now(),
        };
    }

    /// Mark the task as completed
    pub fn mark_completed(&mut self) {
        if let TaskState::Running { started_at } = self.state {
            let completed_at = Instant::now();
            let duration = completed_at.duration_since(started_at);
            self.state = TaskState::Completed {
                started_at,
                completed_at,
                duration,
            };
        }
    }

    /// Mark the task as failed
    pub fn mark_failed(&mut self, error: String) {
        if let TaskState::Running { started_at } = self.state {
            let failed_at = Instant::now();
            let duration = failed_at.duration_since(started_at);
            self.state = TaskState::Failed {
                started_at,
                failed_at,
                duration,
                error,
            };
        }
    }

    /// Mark the task as cancelled
    pub fn mark_cancelled(&mut self) {
        self.state = TaskState::Cancelled;
    }

    /// Increment the attempt counter
    pub fn increment_attempt(&mut self) {
        self.attempt += 1;
    }

    /// Check if the task can be retried
    pub fn can_retry(&self) -> bool {
        self.attempt < self.max_retries && matches!(self.state, TaskState::Failed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct TestTask {
        value: i32,
    }

    #[async_trait::async_trait]
    impl AsyncTask for TestTask {
        type Output = i32;
        type Error = String;

        async fn execute(&self) -> Result<Self::Output, Self::Error> {
            if self.value < 0 {
                Err("Negative value not allowed".to_string())
            } else {
                Ok(self.value * 2)
            }
        }

        fn name(&self) -> String {
            format!("TestTask({})", self.value)
        }
    }

    #[test]
    fn test_task_id_generation() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_state_transitions() {
        let mut metadata = TaskMetadata::new(&TestTask { value: 42 });
        
        assert_eq!(metadata.state, TaskState::Pending);
        assert!(!metadata.state.is_finished());
        assert!(!metadata.state.is_running());

        metadata.mark_started();
        assert!(metadata.state.is_running());
        assert!(!metadata.state.is_finished());

        metadata.mark_completed();
        assert!(metadata.state.is_finished());
        assert!(!metadata.state.is_running());
        assert!(metadata.state.duration().is_some());
    }

    #[tokio::test]
    async fn test_async_task_execution() {
        let task = TestTask { value: 21 };
        let result = task.execute().await;
        assert_eq!(result, Ok(42));

        let failing_task = TestTask { value: -1 };
        let result = failing_task.execute().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_task_result_conversions() {
        let success_result: TaskResult<i32, String> = TaskResult::Success(42);
        assert!(success_result.is_success());
        assert_eq!(success_result.into_result(), Ok(42));

        let error_result: TaskResult<i32, String> = TaskResult::Error("failed".to_string());
        assert!(error_result.is_error());
        assert!(error_result.into_result().is_err());

        let cancelled_result: TaskResult<i32, String> = TaskResult::Cancelled;
        assert!(cancelled_result.is_cancelled());
        assert!(cancelled_result.into_result().is_err());
    }
}