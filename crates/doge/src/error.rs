//! Error types for the Doge task execution framework

use daggy::NodeIndex;
use thiserror::Error;

/// Main error type for Doge operations
#[derive(Error, Debug)]
pub enum DogeError {
    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),

    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Task error: {0}")]
    Task(String),

    #[error("Cancelled")]
    Cancelled,
}

/// Errors related to graph operations
#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Node {0:?} not found in graph")]
    NodeNotFound(NodeIndex),

    #[error("Cycle detected: adding edge from {from:?} to {to:?} would create a cycle")]
    CycleDetected { from: NodeIndex, to: NodeIndex },

    #[error("Task {0:?} is not currently executing")]
    TaskNotExecuting(NodeIndex),

    #[error("Invalid dependency: cannot depend on self")]
    SelfDependency,

    #[error("Graph is empty")]
    EmptyGraph,

    #[error("Graph is in invalid state: {reason}")]
    InvalidState { reason: String },
}

/// Errors related to task execution
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Task execution failed: {task_name}")]
    TaskFailed { task_name: String, source: Box<dyn std::error::Error + Send + Sync> },

    #[error("Task {task_name} timed out after {duration:?}")]
    Timeout { task_name: String, duration: std::time::Duration },

    #[error("Task {task_name} was cancelled")]
    TaskCancelled { task_name: String },

    #[error("Executor is shutdown")]
    ExecutorShutdown,

    #[error("Task {task_name} exceeded maximum retry attempts ({max_retries})")]
    MaxRetriesExceeded { task_name: String, max_retries: u32 },

    #[error("Resource limit exceeded: {resource}")]
    ResourceLimitExceeded { resource: String },

    #[error("Deadlock detected in task dependencies")]
    DeadlockDetected,

    #[error("Task spawn failed: {reason}")]
    SpawnFailed { reason: String },
}

/// Errors related to configuration
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid max concurrent tasks: {value} (must be > 0)")]
    InvalidMaxConcurrentTasks { value: usize },

    #[error("Invalid timeout duration: {duration:?} (must be > 0)")]
    InvalidTimeout { duration: std::time::Duration },

    #[error("Invalid retry configuration: max_retries={max_retries}, delay={delay:?}")]
    InvalidRetryConfig { max_retries: u32, delay: std::time::Duration },

    #[error("Unsupported feature: {feature}")]
    UnsupportedFeature { feature: String },
}

/// Result type alias for Doge operations
pub type DogeResult<T> = Result<T, DogeError>;

/// Result type alias for graph operations
pub type GraphResult<T> = Result<T, GraphError>;

/// Result type alias for execution operations
pub type ExecutionResult<T> = Result<T, ExecutionError>;

/// Result type alias for configuration operations
pub type ConfigResult<T> = Result<T, ConfigError>;

impl DogeError {
    /// Create a new task error
    pub fn task<S: Into<String>>(message: S) -> Self {
        DogeError::Task(message.into())
    }

    /// Create a new execution error from a task failure
    pub fn task_failed<S: Into<String>, E: std::error::Error + Send + Sync + 'static>(
        task_name: S,
        error: E,
    ) -> Self {
        DogeError::Execution(ExecutionError::TaskFailed {
            task_name: task_name.into(),
            source: Box::new(error),
        })
    }

    /// Check if this error indicates cancellation
    pub fn is_cancelled(&self) -> bool {
        matches!(self, DogeError::Cancelled | DogeError::Execution(ExecutionError::TaskCancelled { .. }))
    }

    /// Check if this error indicates a timeout
    pub fn is_timeout(&self) -> bool {
        matches!(self, DogeError::Execution(ExecutionError::Timeout { .. }))
    }

    /// Check if this error indicates a retry exhaustion
    pub fn is_retry_exhausted(&self) -> bool {
        matches!(self, DogeError::Execution(ExecutionError::MaxRetriesExceeded { .. }))
    }
}

impl ExecutionError {
    /// Create a timeout error
    pub fn timeout<S: Into<String>>(task_name: S, duration: std::time::Duration) -> Self {
        ExecutionError::Timeout {
            task_name: task_name.into(),
            duration,
        }
    }

    /// Create a cancellation error
    pub fn cancelled<S: Into<String>>(task_name: S) -> Self {
        ExecutionError::TaskCancelled {
            task_name: task_name.into(),
        }
    }

    /// Create a retry exhaustion error
    pub fn max_retries_exceeded<S: Into<String>>(task_name: S, max_retries: u32) -> Self {
        ExecutionError::MaxRetriesExceeded {
            task_name: task_name.into(),
            max_retries,
        }
    }

    /// Create a resource limit error
    pub fn resource_limit<S: Into<String>>(resource: S) -> Self {
        ExecutionError::ResourceLimitExceeded {
            resource: resource.into(),
        }
    }

    /// Create a spawn failure error
    pub fn spawn_failed<S: Into<String>>(reason: S) -> Self {
        ExecutionError::SpawnFailed {
            reason: reason.into(),
        }
    }
}

impl GraphError {
    /// Create a cycle detection error
    pub fn cycle(from: NodeIndex, to: NodeIndex) -> Self {
        GraphError::CycleDetected { from, to }
    }

    /// Create an invalid state error
    pub fn invalid_state<S: Into<String>>(reason: S) -> Self {
        GraphError::InvalidState {
            reason: reason.into(),
        }
    }
}

impl ConfigError {
    /// Create an invalid max concurrent tasks error
    pub fn invalid_max_concurrent_tasks(value: usize) -> Self {
        ConfigError::InvalidMaxConcurrentTasks { value }
    }

    /// Create an invalid timeout error
    pub fn invalid_timeout(duration: std::time::Duration) -> Self {
        ConfigError::InvalidTimeout { duration }
    }

    /// Create an invalid retry config error
    pub fn invalid_retry_config(max_retries: u32, delay: std::time::Duration) -> Self {
        ConfigError::InvalidRetryConfig { max_retries, delay }
    }

    /// Create an unsupported feature error
    pub fn unsupported_feature<S: Into<String>>(feature: S) -> Self {
        ConfigError::UnsupportedFeature {
            feature: feature.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let task_error = DogeError::task("Test task error");
        assert!(matches!(task_error, DogeError::Task(_)));

        let graph_error = DogeError::Graph(GraphError::EmptyGraph);
        assert!(matches!(graph_error, DogeError::Graph(GraphError::EmptyGraph)));

        let execution_error = DogeError::Execution(ExecutionError::ExecutorShutdown);
        assert!(matches!(execution_error, DogeError::Execution(ExecutionError::ExecutorShutdown)));
    }

    #[test]
    fn test_error_helpers() {
        let cancelled_error = DogeError::Cancelled;
        assert!(cancelled_error.is_cancelled());
        assert!(!cancelled_error.is_timeout());
        assert!(!cancelled_error.is_retry_exhausted());

        let timeout_error = DogeError::Execution(ExecutionError::timeout("test", std::time::Duration::from_secs(5)));
        assert!(!timeout_error.is_cancelled());
        assert!(timeout_error.is_timeout());
        assert!(!timeout_error.is_retry_exhausted());

        let retry_error = DogeError::Execution(ExecutionError::max_retries_exceeded("test", 3));
        assert!(!retry_error.is_cancelled());
        assert!(!retry_error.is_timeout());
        assert!(retry_error.is_retry_exhausted());
    }

    #[test]
    fn test_execution_error_helpers() {
        let timeout = ExecutionError::timeout("test_task", std::time::Duration::from_secs(10));
        if let ExecutionError::Timeout { task_name, duration } = timeout {
            assert_eq!(task_name, "test_task");
            assert_eq!(duration, std::time::Duration::from_secs(10));
        } else {
            panic!("Expected timeout error");
        }

        let cancelled = ExecutionError::cancelled("test_task");
        if let ExecutionError::TaskCancelled { task_name } = cancelled {
            assert_eq!(task_name, "test_task");
        } else {
            panic!("Expected cancellation error");
        }

        let retry_exhausted = ExecutionError::max_retries_exceeded("test_task", 5);
        if let ExecutionError::MaxRetriesExceeded { task_name, max_retries } = retry_exhausted {
            assert_eq!(task_name, "test_task");
            assert_eq!(max_retries, 5);
        } else {
            panic!("Expected retry exhausted error");
        }
    }

    #[test]
    fn test_graph_error_helpers() {
        let node1 = NodeIndex::new(0);
        let node2 = NodeIndex::new(1);
        
        let cycle_error = GraphError::cycle(node1, node2);
        if let GraphError::CycleDetected { from, to } = cycle_error {
            assert_eq!(from, node1);
            assert_eq!(to, node2);
        } else {
            panic!("Expected cycle error");
        }

        let invalid_state = GraphError::invalid_state("test reason");
        if let GraphError::InvalidState { reason } = invalid_state {
            assert_eq!(reason, "test reason");
        } else {
            panic!("Expected invalid state error");
        }
    }

    #[test]
    fn test_config_error_helpers() {
        let invalid_max = ConfigError::invalid_max_concurrent_tasks(0);
        if let ConfigError::InvalidMaxConcurrentTasks { value } = invalid_max {
            assert_eq!(value, 0);
        } else {
            panic!("Expected invalid max concurrent tasks error");
        }

        let invalid_timeout = ConfigError::invalid_timeout(std::time::Duration::from_secs(0));
        if let ConfigError::InvalidTimeout { duration } = invalid_timeout {
            assert_eq!(duration, std::time::Duration::from_secs(0));
        } else {
            panic!("Expected invalid timeout error");
        }

        let unsupported = ConfigError::unsupported_feature("test_feature");
        if let ConfigError::UnsupportedFeature { feature } = unsupported {
            assert_eq!(feature, "test_feature");
        } else {
            panic!("Expected unsupported feature error");
        }
    }
}