//! Custom executor configuration example for the Doge task executor
//!
//! This example demonstrates how to customize the executor with different
//! configurations for various use cases like testing, high throughput, and reliability.

use doge::{async_trait, AsyncTask, DogeExecutor, ExecutorConfig, TaskGraph, TaskPriority};
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A configurable task that can simulate different behaviors
#[derive(Clone, Debug)]
struct ConfigurableTask {
    id: u32,
    duration_ms: u64,
    failure_rate_percent: u32, // 0 to 100
    retry_count: Arc<AtomicU32>,
}

impl std::hash::Hash for ConfigurableTask {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.duration_ms.hash(state);
        self.failure_rate_percent.hash(state);
        // Skip retry_count as it's not part of task identity
    }
}

impl PartialEq for ConfigurableTask {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.duration_ms == other.duration_ms && self.failure_rate_percent == other.failure_rate_percent
    }
}

impl Eq for ConfigurableTask {}

impl ConfigurableTask {
    fn new(id: u32, duration_ms: u64, failure_rate: f32) -> Self {
        Self {
            id,
            duration_ms,
            failure_rate_percent: (failure_rate * 100.0) as u32,
            retry_count: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[async_trait]
impl AsyncTask for ConfigurableTask {
    type Output = String;
    type Error = String;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let attempt = self.retry_count.fetch_add(1, Ordering::SeqCst) + 1;
        
        println!("  🔄 Executing task {} (attempt {})", self.id, attempt);
        
        // Simulate work
        tokio::time::sleep(Duration::from_millis(self.duration_ms)).await;
        
        // Simulate potential failure
        let should_fail = rand::random::<u32>() % 100 < self.failure_rate_percent;
        
        if should_fail && attempt <= 2 { // Fail on first two attempts
            println!("  ❌ Task {} failed on attempt {}", self.id, attempt);
            return Err(format!("Task {} failed (simulated)", self.id));
        }
        
        let result = format!("result_from_task_{}", self.id);
        println!("  ✅ Task {} succeeded on attempt {}: {}", self.id, attempt, result);
        Ok(result)
    }

    fn name(&self) -> String {
        format!("configurable_task_{}", self.id)
    }

    fn priority(&self) -> TaskPriority {
        if self.id <= 3 {
            TaskPriority::High
        } else if self.id <= 6 {
            TaskPriority::Normal
        } else {
            TaskPriority::Low
        }
    }

    fn max_retries(&self) -> u32 {
        3
    }

    fn retry_delay(&self) -> Duration {
        Duration::from_millis(50)
    }
}

async fn run_with_config(
    name: &str,
    config: ExecutorConfig,
    tasks: Vec<ConfigurableTask>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== {} ===", name);
    println!("Configuration:");
    println!("  - Max concurrent tasks: {}", config.max_concurrent_tasks);
    println!("  - Enable retry: {}", config.enable_retry);
    println!("  - Fail fast: {}", config.fail_fast);
    println!("  - Enable caching: {}", config.enable_caching);
    println!("  - Default task timeout: {:?}", config.default_task_timeout);
    
    let mut graph = TaskGraph::new();
    
    // Add all tasks to the graph
    for task in tasks {
        graph.add_task(task);
    }
    
    println!("\nAdded {} tasks to graph", graph.task_count());
    
    // Create executor with custom config
    let executor = DogeExecutor::with_config(config)?;
    
    let start_time = std::time::Instant::now();
    
    let (mut handle, results) = executor.execute_with_progress(graph).await?;
    
    // Monitor progress with simplified output
    let mut last_progress = 0.0;
    tokio::spawn(async move {
        while let Some(event) = handle.next_progress_event().await {
            match event {
                doge::executor::ProgressEvent::Progress(progress) => {
                    let current_progress = (progress.progress_percentage * 100.0) as u32;
                    if current_progress >= last_progress as u32 + 10 {
                        println!("  📊 {}% complete ({} tasks done)", current_progress, progress.completed_tasks);
                        last_progress = current_progress as f64;
                    }
                }
                doge::executor::ProgressEvent::Completed { stats } => {
                    println!("  🎉 Completed in {:?} ({:.2} tasks/sec)", 
                             stats.total_duration, stats.tasks_per_second);
                    println!("  📈 Stats: {} successful, {} failed, {} retries", 
                             stats.successful_tasks, stats.failed_tasks, stats.total_retries);
                }
                doge::executor::ProgressEvent::Failed { error, stats } => {
                    println!("  💥 Failed: {} ({:.2} tasks/sec)", error, stats.tasks_per_second);
                    println!("  📈 Stats: {} successful, {} failed", 
                             stats.successful_tasks, stats.failed_tasks);
                }
                _ => {} // Ignore other events for cleaner output
            }
        }
    });
    
    let duration = start_time.elapsed();
    
    // Count results
    let success_count = results.values().filter(|r| r.is_success()).count();
    let error_count = results.values().filter(|r| r.is_error()).count();
    let cancelled_count = results.values().filter(|r| r.is_cancelled()).count();
    
    println!("Results: {} success, {} error, {} cancelled", success_count, error_count, cancelled_count);
    println!("Total time: {:?}", duration);
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Custom Executor Configuration Example ===");

    // Create a set of tasks with varying characteristics
    let create_task_set = || -> Vec<ConfigurableTask> {
        vec![
            ConfigurableTask::new(1, 50, 0.2),   // Fast, low failure rate
            ConfigurableTask::new(2, 100, 0.3),  // Medium speed, medium failure rate
            ConfigurableTask::new(3, 75, 0.1),   // Fast, very low failure rate
            ConfigurableTask::new(4, 200, 0.4),  // Slow, high failure rate
            ConfigurableTask::new(5, 150, 0.2),  // Medium-slow, low failure rate
            ConfigurableTask::new(6, 80, 0.3),   // Fast-medium, medium failure rate
            ConfigurableTask::new(7, 120, 0.1),  // Medium, very low failure rate
            ConfigurableTask::new(8, 90, 0.2),   // Fast-medium, low failure rate
        ]
    };

    // 1. Testing Configuration - Sequential, fast timeout, no retry
    let testing_config = ExecutorConfig::for_testing();
    run_with_config("Testing Configuration", testing_config, create_task_set()).await?;

    // 2. High Throughput Configuration - Many concurrent tasks, minimal retry
    let high_throughput_config = ExecutorConfig::for_high_throughput();
    run_with_config("High Throughput Configuration", high_throughput_config, create_task_set()).await?;

    // 3. Reliability Configuration - Aggressive retry, longer timeouts
    let reliability_config = ExecutorConfig::for_reliability();
    run_with_config("Reliability Configuration", reliability_config, create_task_set()).await?;

    // 4. Custom Configuration - Balanced approach
    let custom_config = ExecutorConfig::new()
        .with_max_concurrent_tasks(3)?
        .with_retry_enabled(true)
        .with_default_retry_config(2, Duration::from_millis(100))?
        .with_fail_fast(false)
        .with_caching(true)
        .with_default_task_timeout(Duration::from_secs(2))?;
    
    run_with_config("Custom Balanced Configuration", custom_config, create_task_set()).await?;

    // 5. Fail-Fast Configuration - Stop on first failure
    let fail_fast_config = ExecutorConfig::new()
        .with_max_concurrent_tasks(4)?
        .with_fail_fast(true)
        .with_retry_enabled(false)
        .with_default_task_timeout(Duration::from_secs(1))?;
    
    // Use tasks with higher failure rate to demonstrate fail-fast
    let failing_tasks = vec![
        ConfigurableTask::new(1, 50, 0.0),   // This will succeed
        ConfigurableTask::new(2, 100, 0.9),  // This will likely fail quickly
        ConfigurableTask::new(3, 200, 0.0),  // This might not even start
        ConfigurableTask::new(4, 300, 0.0),  // This definitely won't start
    ];
    
    run_with_config("Fail-Fast Configuration", fail_fast_config, failing_tasks).await?;

    println!("\n=== All configurations tested ===");
    println!("\nKey takeaways:");
    println!("  📝 Testing config: Fast, deterministic, good for unit tests");
    println!("  🚀 High throughput: Maximum parallelism, minimal overhead");
    println!("  🛡️  Reliability: Aggressive retry, fault tolerance");
    println!("  ⚖️  Balanced: Good mix of speed and reliability");
    println!("  ⚡ Fail-fast: Quick feedback, stops on first error");

    Ok(())
}