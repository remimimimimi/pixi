//! Main executor for running DAG-based task workflows

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use futures::future::FutureExt;
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info};

use crate::error::{DogeError, DogeResult, ExecutionError};
use crate::graph::{NodeIndex, TaskGraph};
use crate::task::{AsyncTask, TaskId, TaskMetadata, TaskResult};
use crate::types::{ExecutionStats, ExecutorConfig, ProgressInfo};

/// Progress event emitted during execution
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Execution started
    Started { total_tasks: usize },
    /// Task started executing
    TaskStarted { task_id: TaskId, task_name: String },
    /// Task completed successfully
    TaskCompleted { task_id: TaskId, task_name: String, duration: Duration },
    /// Task failed
    TaskFailed { task_id: TaskId, task_name: String, error: String, duration: Duration },
    /// Task was retried
    TaskRetried { task_id: TaskId, task_name: String, attempt: u32 },
    /// Progress update
    Progress(ProgressInfo),
    /// Execution completed
    Completed { stats: ExecutionStats },
    /// Execution failed
    Failed { error: String, stats: ExecutionStats },
}

/// Handle for controlling and monitoring execution
#[derive(Debug)]
pub struct ExecutionHandle<T: AsyncTask> {
    /// Channel for receiving progress events
    pub progress_receiver: mpsc::UnboundedReceiver<ProgressEvent>,
    /// Channel for cancelling execution
    cancellation_sender: oneshot::Sender<()>,
    /// Results of task execution
    results: Arc<DashMap<NodeIndex, TaskResult<T::Output, T::Error>>>,
}

impl<T: AsyncTask> ExecutionHandle<T> {
    /// Cancel the execution
    pub fn cancel(self) {
        let _ = self.cancellation_sender.send(());
    }

    /// Get the result for a specific task
    pub fn get_result(&self, node_index: NodeIndex) -> Option<TaskResult<T::Output, T::Error>> {
        self.results.get(&node_index).map(|entry| entry.value().clone())
    }

    /// Get all results
    pub fn get_all_results(&self) -> HashMap<NodeIndex, TaskResult<T::Output, T::Error>> {
        self.results.iter().map(|entry| (*entry.key(), entry.value().clone())).collect()
    }

    /// Wait for the next progress event
    pub async fn next_progress_event(&mut self) -> Option<ProgressEvent> {
        self.progress_receiver.recv().await
    }
}

/// Main executor for running task graphs
#[derive(Debug)]
pub struct DogeExecutor {
    config: ExecutorConfig,
    cache: Arc<DashMap<u64, Box<dyn std::any::Any + Send + Sync>>>,
}

impl Default for DogeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl DogeExecutor {
    /// Create a new executor with default configuration
    pub fn new() -> Self {
        Self {
            config: ExecutorConfig::default(),
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Create a new executor with custom configuration
    pub fn with_config(config: ExecutorConfig) -> DogeResult<Self> {
        config.validate()?;
        Ok(Self {
            config,
            cache: Arc::new(DashMap::new()),
        })
    }

    /// Get the current configuration
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Execute a task graph and return results
    pub async fn execute<T: AsyncTask>(
        &self,
        graph: TaskGraph<T>,
    ) -> DogeResult<HashMap<NodeIndex, TaskResult<T::Output, T::Error>>> 
    where 
        T::Error: From<String>,
    {
        let (handle, results) = self.execute_with_progress(graph).await?;
        
        // Consume all progress events (we don't need them for this API)
        drop(handle);
        
        Ok(results)
    }

    /// Execute a task graph with progress reporting
    pub async fn execute_with_progress<T: AsyncTask>(
        &self,
        mut graph: TaskGraph<T>,
    ) -> DogeResult<(ExecutionHandle<T>, HashMap<NodeIndex, TaskResult<T::Output, T::Error>>)> 
    where 
        T::Error: From<String>,
    {
        let start_time = Instant::now();
        let total_tasks = graph.task_count();
        
        if total_tasks == 0 {
            return Ok((
                ExecutionHandle {
                    progress_receiver: mpsc::unbounded_channel().1,
                    cancellation_sender: oneshot::channel().0,
                    results: Arc::new(DashMap::new()),
                },
                HashMap::new(),
            ));
        }

        let (progress_sender, progress_receiver) = mpsc::unbounded_channel();
        let (cancellation_sender, mut cancellation_receiver) = oneshot::channel();
        let results = Arc::new(DashMap::new());

        // Send initial progress event
        let _ = progress_sender.send(ProgressEvent::Started { total_tasks });

        // Create semaphore for limiting concurrent tasks
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_tasks));
        let mut active_tasks = 0;
        let mut completed_tasks = 0;
        let mut failed_tasks = 0;
        let mut executing_futures = Vec::new();

        let execution_start = Instant::now();
        let mut stats = ExecutionStats::default();
        stats.total_tasks = total_tasks;

        loop {
            // Check for cancellation
            if cancellation_receiver.try_recv().is_ok() {
                info!("Execution cancelled");
                break;
            }

            // Check for global timeout
            if let Some(global_timeout) = self.config.global_timeout {
                if execution_start.elapsed() > global_timeout {
                    error!("Global timeout exceeded");
                    return Err(DogeError::Execution(ExecutionError::Timeout {
                        task_name: "global execution".to_string(),
                        duration: global_timeout,
                    }));
                }
            }

            // Start new tasks if possible
            while active_tasks < self.config.max_concurrent_tasks {
                if let Some(node_index) = graph.next_ready_task() {
                    let node = graph.get_node_mut(node_index).unwrap();
                    node.metadata.mark_started();
                    
                    let task_future = self.execute_single_task(
                        node_index,
                        node.task.clone(),
                        node.metadata.clone(),
                        semaphore.clone(),
                        progress_sender.clone(),
                        results.clone(),
                    );
                    
                    executing_futures.push(task_future.boxed());
                    active_tasks += 1;
                    stats.peak_concurrent_tasks = stats.peak_concurrent_tasks.max(active_tasks);
                    
                    let _ = progress_sender.send(ProgressEvent::TaskStarted {
                        task_id: node.metadata.id,
                        task_name: node.metadata.name.clone(),
                    });
                } else {
                    break;
                }
            }

            // If no active tasks and no ready tasks, we're done
            if active_tasks == 0 {
                break;
            }

            // Wait for at least one task to complete
            if !executing_futures.is_empty() {
                let (result, _index, remaining_futures) = futures::future::select_all(executing_futures).await;
                executing_futures = remaining_futures;
                active_tasks -= 1;

                match result {
                    Ok((node_index, task_result)) => {
                        match &task_result {
                            TaskResult::Success(_) => {
                                completed_tasks += 1;
                                stats.successful_tasks += 1;
                                graph.mark_completed(node_index)?;
                                
                                if let Some(node) = graph.get_node(node_index) {
                                    if let Some(duration) = node.metadata.state.duration() {
                                        let _ = progress_sender.send(ProgressEvent::TaskCompleted {
                                            task_id: node.metadata.id,
                                            task_name: node.metadata.name.clone(),
                                            duration,
                                        });
                                    }
                                }
                            }
                            TaskResult::Error(err) => {
                                failed_tasks += 1;
                                stats.failed_tasks += 1;
                                graph.mark_failed(node_index, format!("{:?}", err))?;
                                
                                if let Some(node) = graph.get_node(node_index) {
                                    if let Some(duration) = node.metadata.state.duration() {
                                        let _ = progress_sender.send(ProgressEvent::TaskFailed {
                                            task_id: node.metadata.id,
                                            task_name: node.metadata.name.clone(),
                                            error: format!("{:?}", err),
                                            duration,
                                        });
                                    }
                                }

                                if self.config.fail_fast {
                                    error!("Failing fast due to task failure");
                                    break;
                                }
                            }
                            TaskResult::Cancelled => {
                                stats.cancelled_tasks += 1;
                                // Don't update graph state for cancelled tasks
                            }
                        }

                        // Send progress update
                        let mut progress = ProgressInfo::new(total_tasks);
                        progress.update(completed_tasks, failed_tasks, active_tasks);
                        progress.tasks_per_second = completed_tasks as f64 / execution_start.elapsed().as_secs_f64();
                        let _ = progress_sender.send(ProgressEvent::Progress(progress));
                    }
                    Err(err) => {
                        error!("Task execution failed: {:?}", err);
                        failed_tasks += 1;
                        stats.failed_tasks += 1;
                        
                        if self.config.fail_fast {
                            break;
                        }
                    }
                }
            }
        }

        // Calculate final statistics
        stats.total_duration = start_time.elapsed();
        stats.tasks_per_second = (stats.successful_tasks + stats.failed_tasks) as f64 
            / stats.total_duration.as_secs_f64();

        // Send completion event
        let final_results = results.iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect();

        if failed_tasks == 0 {
            let _ = progress_sender.send(ProgressEvent::Completed { stats });
        } else {
            let _ = progress_sender.send(ProgressEvent::Failed {
                error: format!("{} tasks failed", failed_tasks),
                stats,
            });
        }

        let handle = ExecutionHandle {
            progress_receiver,
            cancellation_sender,
            results,
        };

        Ok((handle, final_results))
    }

    /// Execute a single task with retry logic
    async fn execute_single_task<T: AsyncTask>(
        &self,
        node_index: NodeIndex,
        task: T,
        mut metadata: TaskMetadata,
        semaphore: Arc<Semaphore>,
        progress_sender: mpsc::UnboundedSender<ProgressEvent>,
        results: Arc<DashMap<NodeIndex, TaskResult<T::Output, T::Error>>>,
    ) -> DogeResult<(NodeIndex, TaskResult<T::Output, T::Error>)> 
    where 
        T::Error: From<String>,
    {
        let _permit = semaphore.acquire().await
            .map_err(|_| ExecutionError::ResourceLimitExceeded { resource: "semaphore".to_string() })?;

        let mut last_error = None;
        
        for attempt in 0..=metadata.max_retries {
            metadata.attempt = attempt;
            
            if attempt > 0 {
                let _ = progress_sender.send(ProgressEvent::TaskRetried {
                    task_id: metadata.id,
                    task_name: metadata.name.clone(),
                    attempt,
                });
                
                // Wait before retry
                tokio::time::sleep(metadata.retry_delay).await;
            }

            // Check cache first
            if self.config.enable_caching && attempt == 0 {
                let cache_key = self.calculate_cache_key(&task);
                if let Some(cached_result) = self.get_from_cache::<T::Output>(&cache_key) {
                    let task_result = TaskResult::Success(cached_result);
                    results.insert(node_index, task_result.clone());
                    return Ok((node_index, task_result));
                }
            }

            // Execute the task
            let task_future = task.execute();
            let task_result = if let Some(task_timeout) = self.config.default_task_timeout {
                match timeout(task_timeout, task_future).await {
                    Ok(result) => result,
                    Err(_) => {
                        last_error = Some(format!("Task timed out after {:?}", task_timeout));
                        if attempt < metadata.max_retries && task.is_retryable() {
                            continue;
                        } else {
                            return Ok((node_index, TaskResult::Error(
                                format!("Task timed out after {:?}", task_timeout).into()
                            )));
                        }
                    }
                }
            } else {
                task_future.await
            };

            match task_result {
                Ok(output) => {
                    // Cache the result
                    if self.config.enable_caching {
                        let cache_key = self.calculate_cache_key(&task);
                        self.store_in_cache(cache_key, output.clone());
                    }
                    
                    let task_result = TaskResult::Success(output);
                    results.insert(node_index, task_result.clone());
                    return Ok((node_index, task_result));
                }
                Err(err) => {
                    last_error = Some(format!("{:?}", err));
                    
                    if attempt < metadata.max_retries && task.is_retryable() {
                        debug!(
                            task_name = metadata.name,
                            attempt = attempt + 1,
                            max_retries = metadata.max_retries,
                            error = ?err,
                            "Task failed, retrying"
                        );
                        continue;
                    } else {
                        let task_result = TaskResult::Error(err);
                        results.insert(node_index, task_result.clone());
                        return Ok((node_index, task_result));
                    }
                }
            }
        }

        // This should never be reached, but just in case
        let error_msg = last_error.unwrap_or_else(|| "Unknown error".to_string());
        let task_result = TaskResult::Error(error_msg.into());
        results.insert(node_index, task_result.clone());
        Ok((node_index, task_result))
    }

    /// Calculate cache key for a task
    fn calculate_cache_key<T: AsyncTask>(&self, task: &T) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        
        let mut hasher = DefaultHasher::new();
        task.hash(&mut hasher);
        hasher.finish()
    }

    /// Get result from cache
    fn get_from_cache<T: Clone + Send + Sync + 'static>(&self, key: &u64) -> Option<T> {
        self.cache.get(key)?.downcast_ref::<T>().cloned()
    }

    /// Store result in cache
    fn store_in_cache<T: Clone + Send + Sync + 'static>(&self, key: u64, value: T) {
        if self.cache.len() >= self.config.max_cache_size {
            // Simple eviction: remove a random entry
            if let Some(entry) = self.cache.iter().next() {
                let key_to_remove = *entry.key();
                drop(entry);
                self.cache.remove(&key_to_remove);
            }
        }
        
        self.cache.insert(key, Box::new(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::TaskGraph;
    use crate::task::{AsyncTask, TaskPriority};


    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct TestTask {
        id: u32,
        duration_ms: u64,
        should_fail: bool,
        value: i32,
    }

    #[async_trait::async_trait]
    impl AsyncTask for TestTask {
        type Output = i32;
        type Error = String;

        async fn execute(&self) -> Result<Self::Output, Self::Error> {
            tokio::time::sleep(Duration::from_millis(self.duration_ms)).await;
            
            if self.should_fail {
                Err(format!("Task {} failed", self.id))
            } else {
                Ok(self.value)
            }
        }

        fn name(&self) -> String {
            format!("TestTask({})", self.id)
        }

        fn priority(&self) -> TaskPriority {
            TaskPriority::Normal
        }
    }

    #[tokio::test]
    async fn test_simple_execution() {
        let executor = DogeExecutor::new();
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, duration_ms: 10, should_fail: false, value: 42 };
        let task2 = TestTask { id: 2, duration_ms: 10, should_fail: false, value: 84 };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        let results = executor.execute(graph).await.unwrap();
        
        assert_eq!(results.len(), 2);
        assert!(matches!(results[&node1], TaskResult::Success(42)));
        assert!(matches!(results[&node2], TaskResult::Success(84)));
    }

    #[tokio::test]
    async fn test_dependency_execution() {
        let executor = DogeExecutor::new();
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, duration_ms: 50, should_fail: false, value: 1 };
        let task2 = TestTask { id: 2, duration_ms: 10, should_fail: false, value: 2 };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        // task1 must complete before task2
        graph.add_dependency(node1, node2).unwrap();
        
        let start = Instant::now();
        let results = executor.execute(graph).await.unwrap();
        let duration = start.elapsed();
        
        // Should take at least 60ms (50ms + 10ms) due to dependency
        assert!(duration >= Duration::from_millis(55));
        assert!(matches!(results[&node1], TaskResult::Success(1)));
        assert!(matches!(results[&node2], TaskResult::Success(2)));
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let executor = DogeExecutor::new();
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, duration_ms: 50, should_fail: false, value: 1 };
        let task2 = TestTask { id: 2, duration_ms: 50, should_fail: false, value: 2 };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        // No dependencies, should run in parallel
        let start = Instant::now();
        let results = executor.execute(graph).await.unwrap();
        let duration = start.elapsed();
        
        // Should take around 50ms, not 100ms, because tasks run in parallel
        assert!(duration < Duration::from_millis(80));
        assert!(matches!(results[&node1], TaskResult::Success(1)));
        assert!(matches!(results[&node2], TaskResult::Success(2)));
    }

    #[tokio::test]
    async fn test_task_failure() {
        let executor = DogeExecutor::new();
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, duration_ms: 10, should_fail: false, value: 1 };
        let task2 = TestTask { id: 2, duration_ms: 10, should_fail: true, value: 2 };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        let results = executor.execute(graph).await.unwrap();
        
        assert!(matches!(results[&node1], TaskResult::Success(1)));
        assert!(matches!(results[&node2], TaskResult::Error(_)));
    }

    #[tokio::test]
    async fn test_fail_fast() {
        let config = ExecutorConfig::default().with_fail_fast(true);
        let executor = DogeExecutor::with_config(config).unwrap();
        let mut graph = TaskGraph::new();
        
        // Create a task that fails quickly and one that takes longer
        let fast_fail_task = TestTask { id: 1, duration_ms: 10, should_fail: true, value: 1 };
        let slow_task = TestTask { id: 2, duration_ms: 1000, should_fail: false, value: 2 };
        
        let node1 = graph.add_task(fast_fail_task);
        let node2 = graph.add_task(slow_task);
        
        let start = Instant::now();
        let results = executor.execute(graph).await.unwrap();
        let duration = start.elapsed();
        
        // Should complete quickly due to fail-fast
        assert!(duration < Duration::from_millis(500));
        assert!(matches!(results[&node1], TaskResult::Error(_)));
        
        // The slow task might not have completed
        if results.contains_key(&node2) {
            // If it's in results, it was either cancelled or completed
            assert!(matches!(results[&node2], TaskResult::Success(_) | TaskResult::Cancelled));
        }
    }

    #[tokio::test]
    async fn test_concurrent_limit() {
        let config = ExecutorConfig::default()
            .with_max_concurrent_tasks(1).unwrap();
        let executor = DogeExecutor::with_config(config).unwrap();
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, duration_ms: 50, should_fail: false, value: 1 };
        let task2 = TestTask { id: 2, duration_ms: 50, should_fail: false, value: 2 };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        let start = Instant::now();
        let results = executor.execute(graph).await.unwrap();
        let duration = start.elapsed();
        
        // Should take at least 100ms because max_concurrent_tasks=1
        assert!(duration >= Duration::from_millis(90));
        assert!(matches!(results[&node1], TaskResult::Success(1)));
        assert!(matches!(results[&node2], TaskResult::Success(2)));
    }

    #[tokio::test]
    async fn test_deduplication() {
        let executor = DogeExecutor::new();
        let mut graph = TaskGraph::new();
        
        let task = TestTask { id: 1, duration_ms: 10, should_fail: false, value: 42 };
        
        // Add the same task multiple times
        let node1 = graph.add_task(task.clone());
        let node2 = graph.add_task(task.clone());
        let node3 = graph.add_task(task);
        
        // All should return the same node due to deduplication
        assert_eq!(node1, node2);
        assert_eq!(node2, node3);
        assert_eq!(graph.task_count(), 1);
        
        let results = executor.execute(graph).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(matches!(results[&node1], TaskResult::Success(42)));
    }

    #[tokio::test]
    async fn test_caching() {
        let config = ExecutorConfig::default().with_caching(true);
        let executor = DogeExecutor::with_config(config).unwrap();
        
        // First execution
        let mut graph1 = TaskGraph::new();
        let task = TestTask { id: 1, duration_ms: 100, should_fail: false, value: 42 };
        let node1 = graph1.add_task(task.clone());
        
        let start = Instant::now();
        let results1 = executor.execute(graph1).await.unwrap();
        let duration1 = start.elapsed();
        
        assert!(matches!(results1[&node1], TaskResult::Success(42)));
        assert!(duration1 >= Duration::from_millis(90));
        
        // Second execution with same task should be faster due to caching
        let mut graph2 = TaskGraph::new();
        let node2 = graph2.add_task(task);
        
        let start = Instant::now();
        let results2 = executor.execute(graph2).await.unwrap();
        let duration2 = start.elapsed();
        
        assert!(matches!(results2[&node2], TaskResult::Success(42)));
        // Should be much faster due to cache hit
        assert!(duration2 < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_progress_reporting() {
        let executor = DogeExecutor::new();
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, duration_ms: 20, should_fail: false, value: 1 };
        let task2 = TestTask { id: 2, duration_ms: 20, should_fail: false, value: 2 };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        let (mut handle, results) = executor.execute_with_progress(graph).await.unwrap();
        
        let mut events = Vec::new();
        while let Some(event) = handle.next_progress_event().await {
            events.push(event);
        }
        
        // Should have received various progress events
        assert!(!events.is_empty());
        
        // First event should be Started
        assert!(matches!(events[0], ProgressEvent::Started { total_tasks: 2 }));
        
        // Last event should be Completed
        if let Some(last_event) = events.last() {
            assert!(matches!(last_event, ProgressEvent::Completed { .. }));
        }
        
        assert!(matches!(results[&node1], TaskResult::Success(1)));
        assert!(matches!(results[&node2], TaskResult::Success(2)));
    }
}
