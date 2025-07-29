//! Common types and configuration for the Doge executor

use std::time::Duration;
use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, ConfigResult};

/// Configuration for the Doge executor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// Maximum number of tasks that can execute concurrently
    pub max_concurrent_tasks: usize,
    
    /// Global timeout for task execution (None for no timeout)
    pub global_timeout: Option<Duration>,
    
    /// Default timeout for individual tasks (None for no timeout)
    pub default_task_timeout: Option<Duration>,
    
    /// Whether to enable task retry on failure
    pub enable_retry: bool,
    
    /// Default maximum number of retries for failed tasks
    pub default_max_retries: u32,
    
    /// Default delay between retry attempts
    pub default_retry_delay: Duration,
    
    /// Whether to fail fast on the first task failure
    pub fail_fast: bool,
    
    /// Enable progress reporting
    pub enable_progress_reporting: bool,
    
    /// Buffer size for progress reporting channel
    pub progress_buffer_size: usize,
    
    /// Whether to enable task result caching
    pub enable_caching: bool,
    
    /// Maximum number of cached results to keep in memory
    pub max_cache_size: usize,
    
    /// Enable graceful shutdown on cancellation
    pub graceful_shutdown: bool,
    
    /// Timeout for graceful shutdown
    pub graceful_shutdown_timeout: Duration,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: num_cpus::get(),
            global_timeout: None,
            default_task_timeout: Some(Duration::from_secs(300)), // 5 minutes
            enable_retry: true,
            default_max_retries: 3,
            default_retry_delay: Duration::from_millis(100),
            fail_fast: false,
            enable_progress_reporting: true,
            progress_buffer_size: 1000,
            enable_caching: true,
            max_cache_size: 10000,
            graceful_shutdown: true,
            graceful_shutdown_timeout: Duration::from_secs(30),
        }
    }
}

impl ExecutorConfig {
    /// Create a new executor configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of concurrent tasks
    pub fn with_max_concurrent_tasks(mut self, max_concurrent_tasks: usize) -> ConfigResult<Self> {
        if max_concurrent_tasks == 0 {
            return Err(ConfigError::invalid_max_concurrent_tasks(max_concurrent_tasks));
        }
        self.max_concurrent_tasks = max_concurrent_tasks;
        Ok(self)
    }

    /// Set the global timeout
    pub fn with_global_timeout(mut self, timeout: Duration) -> ConfigResult<Self> {
        if timeout.is_zero() {
            return Err(ConfigError::invalid_timeout(timeout));
        }
        self.global_timeout = Some(timeout);
        Ok(self)
    }

    /// Set the default task timeout
    pub fn with_default_task_timeout(mut self, timeout: Duration) -> ConfigResult<Self> {
        if timeout.is_zero() {
            return Err(ConfigError::invalid_timeout(timeout));
        }
        self.default_task_timeout = Some(timeout);
        Ok(self)
    }

    /// Enable or disable task retry
    pub fn with_retry_enabled(mut self, enabled: bool) -> Self {
        self.enable_retry = enabled;
        self
    }

    /// Set the default retry configuration
    pub fn with_default_retry_config(mut self, max_retries: u32, delay: Duration) -> ConfigResult<Self> {
        if delay.is_zero() {
            return Err(ConfigError::invalid_retry_config(max_retries, delay));
        }
        self.default_max_retries = max_retries;
        self.default_retry_delay = delay;
        Ok(self)
    }

    /// Enable or disable fail-fast behavior
    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Enable or disable progress reporting
    pub fn with_progress_reporting(mut self, enabled: bool) -> Self {
        self.enable_progress_reporting = enabled;
        self
    }

    /// Set the progress buffer size
    pub fn with_progress_buffer_size(mut self, size: usize) -> Self {
        self.progress_buffer_size = size;
        self
    }

    /// Enable or disable caching
    pub fn with_caching(mut self, enabled: bool) -> Self {
        self.enable_caching = enabled;
        self
    }

    /// Set the maximum cache size
    pub fn with_max_cache_size(mut self, size: usize) -> Self {
        self.max_cache_size = size;
        self
    }

    /// Set graceful shutdown configuration
    pub fn with_graceful_shutdown(mut self, enabled: bool, timeout: Duration) -> ConfigResult<Self> {
        if enabled && timeout.is_zero() {
            return Err(ConfigError::invalid_timeout(timeout));
        }
        self.graceful_shutdown = enabled;
        self.graceful_shutdown_timeout = timeout;
        Ok(self)
    }

    /// Validate the configuration
    pub fn validate(&self) -> ConfigResult<()> {
        if self.max_concurrent_tasks == 0 {
            return Err(ConfigError::invalid_max_concurrent_tasks(self.max_concurrent_tasks));
        }

        if let Some(timeout) = self.global_timeout {
            if timeout.is_zero() {
                return Err(ConfigError::invalid_timeout(timeout));
            }
        }

        if let Some(timeout) = self.default_task_timeout {
            if timeout.is_zero() {
                return Err(ConfigError::invalid_timeout(timeout));
            }
        }

        if self.default_retry_delay.is_zero() {
            return Err(ConfigError::invalid_retry_config(self.default_max_retries, self.default_retry_delay));
        }

        if self.graceful_shutdown && self.graceful_shutdown_timeout.is_zero() {
            return Err(ConfigError::invalid_timeout(self.graceful_shutdown_timeout));
        }

        Ok(())
    }

    /// Create a configuration optimized for testing
    pub fn for_testing() -> Self {
        Self {
            max_concurrent_tasks: 1,
            global_timeout: Some(Duration::from_secs(10)),
            default_task_timeout: Some(Duration::from_secs(5)),
            enable_retry: false,
            default_max_retries: 0,
            default_retry_delay: Duration::from_millis(1),
            fail_fast: true,
            enable_progress_reporting: false,
            progress_buffer_size: 10,
            enable_caching: false,
            max_cache_size: 100,
            graceful_shutdown: false,
            graceful_shutdown_timeout: Duration::from_secs(1),
        }
    }

    /// Create a configuration optimized for high-throughput scenarios
    pub fn for_high_throughput() -> Self {
        Self {
            max_concurrent_tasks: num_cpus::get() * 4,
            global_timeout: None,
            default_task_timeout: Some(Duration::from_secs(60)),
            enable_retry: true,
            default_max_retries: 1,
            default_retry_delay: Duration::from_millis(10),
            fail_fast: false,
            enable_progress_reporting: false,
            progress_buffer_size: 10000,
            enable_caching: true,
            max_cache_size: 100000,
            graceful_shutdown: true,
            graceful_shutdown_timeout: Duration::from_secs(5),
        }
    }

    /// Create a configuration optimized for reliability
    pub fn for_reliability() -> Self {
        Self {
            max_concurrent_tasks: num_cpus::get(),
            global_timeout: Some(Duration::from_secs(3600)), // 1 hour
            default_task_timeout: Some(Duration::from_secs(600)), // 10 minutes
            enable_retry: true,
            default_max_retries: 5,
            default_retry_delay: Duration::from_secs(1),
            fail_fast: false,
            enable_progress_reporting: true,
            progress_buffer_size: 1000,
            enable_caching: true,
            max_cache_size: 50000,
            graceful_shutdown: true,
            graceful_shutdown_timeout: Duration::from_secs(60),
        }
    }
}

/// Resource limits for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage per task (in bytes)
    pub max_memory_per_task: Option<usize>,
    
    /// Maximum total memory usage (in bytes)
    pub max_total_memory: Option<usize>,
    
    /// Maximum CPU time per task
    pub max_cpu_time_per_task: Option<Duration>,
    
    /// Maximum number of file descriptors per task
    pub max_file_descriptors_per_task: Option<usize>,
    
    /// Maximum disk usage per task (in bytes)
    pub max_disk_usage_per_task: Option<usize>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_per_task: None,
            max_total_memory: None,
            max_cpu_time_per_task: None,
            max_file_descriptors_per_task: None,
            max_disk_usage_per_task: None,
        }
    }
}

/// Progress information for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressInfo {
    /// Total number of tasks
    pub total_tasks: usize,
    
    /// Number of completed tasks
    pub completed_tasks: usize,
    
    /// Number of failed tasks
    pub failed_tasks: usize,
    
    /// Number of currently executing tasks
    pub executing_tasks: usize,
    
    /// Number of tasks waiting to execute
    pub pending_tasks: usize,
    
    /// Overall progress percentage (0.0 to 1.0)
    pub progress_percentage: f64,
    
    /// Estimated time remaining
    pub estimated_time_remaining: Option<Duration>,
    
    /// Tasks per second
    pub tasks_per_second: f64,
}

impl ProgressInfo {
    /// Create new progress info
    pub fn new(total_tasks: usize) -> Self {
        Self {
            total_tasks,
            completed_tasks: 0,
            failed_tasks: 0,
            executing_tasks: 0,
            pending_tasks: total_tasks,
            progress_percentage: 0.0,
            estimated_time_remaining: None,
            tasks_per_second: 0.0,
        }
    }

    /// Update progress info
    pub fn update(&mut self, completed: usize, failed: usize, executing: usize) {
        self.completed_tasks = completed;
        self.failed_tasks = failed;
        self.executing_tasks = executing;
        self.pending_tasks = self.total_tasks.saturating_sub(completed + failed + executing);
        
        if self.total_tasks > 0 {
            self.progress_percentage = (completed + failed) as f64 / self.total_tasks as f64;
        }
    }

    /// Check if execution is complete
    pub fn is_complete(&self) -> bool {
        self.completed_tasks + self.failed_tasks == self.total_tasks
    }

    /// Check if all tasks completed successfully
    pub fn is_successful(&self) -> bool {
        self.is_complete() && self.failed_tasks == 0
    }
}

/// Execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total execution time
    pub total_duration: Duration,
    
    /// Number of tasks executed
    pub total_tasks: usize,
    
    /// Number of successful tasks
    pub successful_tasks: usize,
    
    /// Number of failed tasks
    pub failed_tasks: usize,
    
    /// Number of cancelled tasks
    pub cancelled_tasks: usize,
    
    /// Average task duration
    pub average_task_duration: Duration,
    
    /// Maximum task duration
    pub max_task_duration: Duration,
    
    /// Minimum task duration
    pub min_task_duration: Duration,
    
    /// Tasks per second
    pub tasks_per_second: f64,
    
    /// Peak concurrent tasks
    pub peak_concurrent_tasks: usize,
    
    /// Total retry attempts
    pub total_retries: u32,
    
    /// Cache hit rate (if caching enabled)
    pub cache_hit_rate: Option<f64>,
}

impl Default for ExecutionStats {
    fn default() -> Self {
        Self {
            total_duration: Duration::from_secs(0),
            total_tasks: 0,
            successful_tasks: 0,
            failed_tasks: 0,
            cancelled_tasks: 0,
            average_task_duration: Duration::from_secs(0),
            max_task_duration: Duration::from_secs(0),
            min_task_duration: Duration::from_secs(0),
            tasks_per_second: 0.0,
            peak_concurrent_tasks: 0,
            total_retries: 0,
            cache_hit_rate: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ExecutorConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.max_concurrent_tasks > 0);
        assert!(config.enable_retry);
        assert!(!config.fail_fast);
    }

    #[test]
    fn test_config_builder() {
        let config = ExecutorConfig::new()
            .with_max_concurrent_tasks(4).unwrap()
            .with_fail_fast(true)
            .with_retry_enabled(false);
        
        assert_eq!(config.max_concurrent_tasks, 4);
        assert!(config.fail_fast);
        assert!(!config.enable_retry);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let result = ExecutorConfig::new().with_max_concurrent_tasks(0);
        assert!(result.is_err());

        let result = ExecutorConfig::new().with_global_timeout(Duration::from_secs(0));
        assert!(result.is_err());

        let result = ExecutorConfig::new().with_default_retry_config(3, Duration::from_secs(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_preset_configs() {
        let testing_config = ExecutorConfig::for_testing();
        assert!(testing_config.validate().is_ok());
        assert_eq!(testing_config.max_concurrent_tasks, 1);
        assert!(testing_config.fail_fast);
        assert!(!testing_config.enable_retry);

        let high_throughput_config = ExecutorConfig::for_high_throughput();
        assert!(high_throughput_config.validate().is_ok());
        assert!(high_throughput_config.max_concurrent_tasks >= num_cpus::get());
        assert!(!high_throughput_config.enable_progress_reporting);

        let reliability_config = ExecutorConfig::for_reliability();
        assert!(reliability_config.validate().is_ok());
        assert_eq!(reliability_config.default_max_retries, 5);
        assert!(reliability_config.enable_progress_reporting);
    }

    #[test]
    fn test_progress_info() {
        let mut progress = ProgressInfo::new(10);
        assert_eq!(progress.total_tasks, 10);
        assert_eq!(progress.pending_tasks, 10);
        assert_eq!(progress.progress_percentage, 0.0);
        assert!(!progress.is_complete());

        progress.update(5, 0, 2);
        assert_eq!(progress.completed_tasks, 5);
        assert_eq!(progress.executing_tasks, 2);
        assert_eq!(progress.pending_tasks, 3);
        assert_eq!(progress.progress_percentage, 0.5);
        assert!(!progress.is_complete());

        progress.update(8, 2, 0);
        assert_eq!(progress.completed_tasks, 8);
        assert_eq!(progress.failed_tasks, 2);
        assert_eq!(progress.pending_tasks, 0);
        assert_eq!(progress.progress_percentage, 1.0);
        assert!(progress.is_complete());
        assert!(!progress.is_successful()); // Has failures

        progress.update(10, 0, 0);
        assert!(progress.is_complete());
        assert!(progress.is_successful()); // No failures
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits::default();
        assert!(limits.max_memory_per_task.is_none());
        assert!(limits.max_total_memory.is_none());
        assert!(limits.max_cpu_time_per_task.is_none());
    }

    #[test]
    fn test_execution_stats() {
        let stats = ExecutionStats::default();
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.successful_tasks, 0);
        assert_eq!(stats.failed_tasks, 0);
        assert_eq!(stats.tasks_per_second, 0.0);
    }
}