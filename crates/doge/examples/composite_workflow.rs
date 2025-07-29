//! Composite workflow example for the Doge task executor
//!
//! This example demonstrates how to create complex workflows with dependencies
//! between tasks, showing how the executor respects dependency ordering.

use doge::{async_trait, AsyncTask, DogeExecutor, TaskGraph, TaskPriority};
use std::time::Duration;

/// A unified task enum for the workflow
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum WorkflowTask {
    FetchData { source: String },
    ProcessData { id: String },
    Aggregate { name: String },
    Validate { target: String },
}

#[async_trait]
impl AsyncTask for WorkflowTask {
    type Output = String;
    type Error = String;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        match self {
            WorkflowTask::FetchData { source } => {
                println!("  🔄 Fetching data from {}...", source);
                
                // Simulate network delay
                tokio::time::sleep(Duration::from_millis(200)).await;
                
                if source == "broken_source" {
                    return Err(format!("Failed to fetch from {}", source));
                }
                
                let result = format!("fetched_data_from_{}", source);
                println!("  ✅ Successfully fetched data from {}: {}", source, result);
                Ok(result)
            }
            WorkflowTask::ProcessData { id } => {
                println!("  🔄 Processing data for {}...", id);
                
                // Simulate processing time
                tokio::time::sleep(Duration::from_millis(300)).await;
                
                let result = format!("processed_result_{}", id);
                println!("  ✅ Processing completed for {}: {}", id, result);
                Ok(result)
            }
            WorkflowTask::Aggregate { name } => {
                println!("  🔄 Aggregating results for {}...", name);
                
                // Simulate aggregation work
                tokio::time::sleep(Duration::from_millis(150)).await;
                
                let result = format!("aggregated_{}", name);
                println!("  ✅ Aggregation completed: {}", result);
                Ok(result)
            }
            WorkflowTask::Validate { target } => {
                println!("  🔄 Validating {}...", target);
                
                // Simulate validation
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                println!("  ✅ Validation passed for {}", target);
                Ok(format!("validated_{}", target))
            }
        }
    }

    fn name(&self) -> String {
        match self {
            WorkflowTask::FetchData { source } => format!("fetch_data({})", source),
            WorkflowTask::ProcessData { id } => format!("process_data({})", id),
            WorkflowTask::Aggregate { name } => format!("aggregate({})", name),
            WorkflowTask::Validate { target } => format!("validate({})", target),
        }
    }

    fn priority(&self) -> TaskPriority {
        match self {
            WorkflowTask::FetchData { .. } => TaskPriority::High,
            WorkflowTask::ProcessData { .. } => TaskPriority::Normal,
            WorkflowTask::Aggregate { .. } => TaskPriority::Low,
            WorkflowTask::Validate { .. } => TaskPriority::Critical,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Composite Workflow Example ===\n");

    // Create a new task graph
    let mut graph = TaskGraph::new();

    println!("Building workflow:");
    println!("  1. Fetch data from multiple sources (parallel)");
    println!("  2. Process each data source (depends on fetch)");
    println!("  3. Aggregate all processed results (depends on all processing)");
    println!("  4. Validate the final result (depends on aggregation)");
    println!();

    // Phase 1: Data fetching (can run in parallel)
    let fetch_api = WorkflowTask::FetchData { source: "api".to_string() };
    let fetch_db = WorkflowTask::FetchData { source: "database".to_string() };
    let fetch_file = WorkflowTask::FetchData { source: "file".to_string() };

    let fetch_api_node = graph.add_task(fetch_api);
    let fetch_db_node = graph.add_task(fetch_db);
    let fetch_file_node = graph.add_task(fetch_file);

    // Phase 2: Data processing (depends on corresponding fetch tasks)
    let process_api = WorkflowTask::ProcessData { id: "api_data".to_string() };
    let process_db = WorkflowTask::ProcessData { id: "db_data".to_string() };
    let process_file = WorkflowTask::ProcessData { id: "file_data".to_string() };

    let process_api_node = graph.add_task(process_api);
    let process_db_node = graph.add_task(process_db);
    let process_file_node = graph.add_task(process_file);

    // Add dependencies: processing depends on fetching
    graph.add_dependency(fetch_api_node, process_api_node)?;
    graph.add_dependency(fetch_db_node, process_db_node)?;
    graph.add_dependency(fetch_file_node, process_file_node)?;

    // Phase 3: Aggregation (depends on all processing tasks)
    let aggregate = WorkflowTask::Aggregate { name: "final_results".to_string() };
    let aggregate_node = graph.add_task(aggregate);

    graph.add_dependency(process_api_node, aggregate_node)?;
    graph.add_dependency(process_db_node, aggregate_node)?;
    graph.add_dependency(process_file_node, aggregate_node)?;

    // Phase 4: Validation (depends on aggregation)
    let validate = WorkflowTask::Validate { target: "final_results".to_string() };
    let validate_node = graph.add_task(validate);

    graph.add_dependency(aggregate_node, validate_node)?;

    println!("Created workflow with {} tasks", graph.task_count());

    // Print graph statistics
    let stats = graph.stats();
    println!("Graph stats: {:?}", stats);

    // Create executor with progress reporting
    let executor = DogeExecutor::new();
    
    println!("\n🚀 Starting workflow execution...\n");
    let start_time = std::time::Instant::now();
    
    let (mut handle, results) = executor.execute_with_progress(graph).await?;
    
    // Monitor progress
    tokio::spawn(async move {
        while let Some(event) = handle.next_progress_event().await {
            match event {
                doge::executor::ProgressEvent::Started { total_tasks } => {
                    println!("📊 Execution started - {} total tasks", total_tasks);
                }
                doge::executor::ProgressEvent::TaskStarted { task_name, .. } => {
                    println!("▶️  Started: {}", task_name);
                }
                doge::executor::ProgressEvent::TaskCompleted { task_name, duration, .. } => {
                    println!("✅ Completed: {} (took {:?})", task_name, duration);
                }
                doge::executor::ProgressEvent::TaskFailed { task_name, error, .. } => {
                    println!("❌ Failed: {} - {}", task_name, error);
                }
                doge::executor::ProgressEvent::Progress(progress) => {
                    println!("📈 Progress: {:.1}% ({}/{} completed, {} executing)", 
                             progress.progress_percentage * 100.0,
                             progress.completed_tasks,
                             progress.total_tasks,
                             progress.executing_tasks);
                }
                doge::executor::ProgressEvent::Completed { stats } => {
                    println!("🎉 Execution completed successfully!");
                    println!("   Total time: {:?}", stats.total_duration);
                    println!("   Tasks/second: {:.2}", stats.tasks_per_second);
                    println!("   Peak concurrent: {}", stats.peak_concurrent_tasks);
                }
                doge::executor::ProgressEvent::Failed { error, stats } => {
                    println!("💥 Execution failed: {}", error);
                    println!("   Total time: {:?}", stats.total_duration);
                    println!("   Successful: {}", stats.successful_tasks);
                    println!("   Failed: {}", stats.failed_tasks);
                }
                _ => {}
            }
        }
    });

    let duration = start_time.elapsed();
    
    println!("\n=== Final Results ===");
    println!("Total execution time: {:?}", duration);
    
    let mut success_count = 0;
    let mut error_count = 0;
    
    for (node_index, result) in results {
        match result {
            doge::TaskResult::Success(output) => {
                success_count += 1;
                println!("✅ Node {:?}: Success", node_index);
            }
            doge::TaskResult::Error(error) => {
                error_count += 1;
                println!("❌ Node {:?}: Error -> {:?}", node_index, error);
            }
            doge::TaskResult::Cancelled => {
                println!("⏹️  Node {:?}: Cancelled", node_index);
            }
        }
    }
    
    println!("\nSummary: {} successful, {} failed", success_count, error_count);
    
    if error_count == 0 {
        println!("🎉 All tasks completed successfully!");
    } else {
        println!("⚠️  Some tasks failed.");
    }

    println!("\n=== Workflow completed ===");
    Ok(())
}