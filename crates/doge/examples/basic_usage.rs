//! Basic usage example for the Doge task executor
//!
//! This example demonstrates how to create simple tasks, add them to a graph,
//! and execute them with the Doge executor.

use doge::{async_trait, AsyncTask, DogeExecutor, TaskGraph};
use std::time::Duration;

/// A unified task that can do different operations
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MathTask {
    Add { a: i32, b: i32 },
    Multiply { a: i32, b: i32 },
}

#[async_trait]
impl AsyncTask for MathTask {
    type Output = i32;
    type Error = String;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        match self {
            MathTask::Add { a, b } => {
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(a + b)
            }
            MathTask::Multiply { a, b } => {
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(150)).await;
                Ok(a * b)
            }
        }
    }

    fn name(&self) -> String {
        match self {
            MathTask::Add { a, b } => format!("add({}, {})", a, b),
            MathTask::Multiply { a, b } => format!("multiply({}, {})", a, b),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Basic Doge Usage Example ===\n");

    // Create a new task graph
    let mut graph = TaskGraph::new();

    // Create some tasks
    let add_task = MathTask::Add { a: 10, b: 20 };
    let multiply_task = MathTask::Multiply { a: 3, b: 7 };
    let another_add_task = MathTask::Add { a: 5, b: 15 };

    println!("Creating tasks:");
    println!("  - {}", add_task.name());
    println!("  - {}", multiply_task.name());
    println!("  - {}", another_add_task.name());

    // Add tasks to the graph
    let add_node = graph.add_task(add_task);
    let multiply_node = graph.add_task(multiply_task);
    let another_add_node = graph.add_task(another_add_task);

    println!("\nAdded {} tasks to the graph", graph.task_count());

    // Create executor and run the tasks
    let executor = DogeExecutor::new();
    
    println!("\nExecuting tasks...");
    let start_time = std::time::Instant::now();
    
    let results = executor.execute(graph).await?;
    
    let duration = start_time.elapsed();
    
    println!("\nExecution completed in {:?}", duration);
    println!("\nResults:");
    
    for (node_index, result) in results {
        match result {
            doge::TaskResult::Success(output) => {
                println!("  Node {:?}: Success -> {}", node_index, output);
            }
            doge::TaskResult::Error(error) => {
                println!("  Node {:?}: Error -> {:?}", node_index, error);
            }
            doge::TaskResult::Cancelled => {
                println!("  Node {:?}: Cancelled", node_index);
            }
        }
    }

    println!("\n=== Example completed ===");
    Ok(())
}