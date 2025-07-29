//! Example demonstrating data flow between dependent tasks
//!
//! This example shows how tasks can access the outputs of their dependencies
//! through the TaskContext, similar to how pixi_command_dispatcher passes
//! outputs from pin_and_checkout to source_metadata tasks.

use doge::{async_trait, AsyncTask, DogeExecutor, TaskGraph, TaskContext, TaskPriority};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Output data that can be passed between tasks
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TaskOutput {
    ProducerData(HashMap<String, i32>),
    ConsumerResult(String),
    AggregateResult(String),
}

/// Unified task type that can represent different kinds of operations
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WorkflowTask {
    DataProducer { name: String, value: i32 },
    DataConsumer { name: String },
    Aggregator { name: String },
}

#[async_trait]
impl AsyncTask for WorkflowTask {
    type Output = TaskOutput;
    type Error = String;

    async fn execute_with_context(&self, context: &TaskContext) -> Result<Self::Output, Self::Error> {
        match self {
            WorkflowTask::DataProducer { name, value } => {
                let mut data = HashMap::new();
                data.insert(name.clone(), *value);
                data.insert("timestamp".to_string(), 12345);
                
                println!("DataProducer '{}' producing data: {:?}", name, data);
                Ok(TaskOutput::ProducerData(data))
            }
            
            WorkflowTask::DataConsumer { name } => {
                println!("DataConsumer '{}' accessing dependency results...", name);
                
                let mut combined_data = HashMap::new();
                let mut total_value = 0;
                
                // Access all dependency results
                for node_index in context.dependency_nodes() {
                    if let Some(TaskOutput::ProducerData(dep_data)) = context.get_dependency_result::<TaskOutput>(node_index) {
                        println!("  Found dependency data: {:?}", dep_data);
                        
                        // Combine the data
                        for (key, value) in dep_data {
                            combined_data.insert(key.clone(), value);
                            if key != "timestamp" {
                                total_value += value;
                            }
                        }
                    }
                }
                
                let result = format!(
                    "Consumer '{}' processed {} dependencies with total value: {} (combined: {:?})",
                    name,
                    context.dependency_count(),
                    total_value,
                    combined_data
                );
                
                println!("DataConsumer result: {}", result);
                Ok(TaskOutput::ConsumerResult(result))
            }
            
            WorkflowTask::Aggregator { name } => {
                println!("Aggregator '{}' aggregating results...", name);
                
                let mut results = Vec::new();
                
                // Collect all dependency results
                for node_index in context.dependency_nodes() {
                    if let Some(TaskOutput::ConsumerResult(dep_result)) = context.get_dependency_result::<TaskOutput>(node_index) {
                        results.push(dep_result);
                    }
                }
                
                let aggregate = format!(
                    "Aggregator '{}' combined {} results: [{}]",
                    name,
                    results.len(),
                    results.join("; ")
                );
                
                println!("Aggregator result: {}", aggregate);
                Ok(TaskOutput::AggregateResult(aggregate))
            }
        }
    }

    // Fallback for tasks that don't need context
    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        self.execute_with_context(&TaskContext::new()).await
    }

    fn name(&self) -> String {
        match self {
            WorkflowTask::DataProducer { name, .. } => format!("DataProducer({})", name),
            WorkflowTask::DataConsumer { name } => format!("DataConsumer({})", name),
            WorkflowTask::Aggregator { name } => format!("Aggregator({})", name),
        }
    }

    fn priority(&self) -> TaskPriority {
        match self {
            WorkflowTask::DataProducer { .. } => TaskPriority::High,
            WorkflowTask::DataConsumer { .. } => TaskPriority::Normal,
            WorkflowTask::Aggregator { .. } => TaskPriority::Low,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Dependency Data Flow Example ===\n");
    
    // Create a graph that mimics pixi_command_dispatcher pattern:
    // Producer1 ──┐
    //             ├──> Consumer1 ──┐
    // Producer2 ──┘                ├──> Aggregator
    // Producer3 ────> Consumer2 ────┘
    
    let mut graph = TaskGraph::new();
    
    // Create producer tasks
    let producer1 = WorkflowTask::DataProducer { name: "source1".to_string(), value: 10 };
    let producer2 = WorkflowTask::DataProducer { name: "source2".to_string(), value: 20 };
    let producer3 = WorkflowTask::DataProducer { name: "source3".to_string(), value: 30 };
    
    let producer1_node = graph.add_task(producer1);
    let producer2_node = graph.add_task(producer2);
    let producer3_node = graph.add_task(producer3);
    
    // Create consumer tasks that depend on producers
    let consumer1 = WorkflowTask::DataConsumer { name: "metadata_processor1".to_string() };
    let consumer2 = WorkflowTask::DataConsumer { name: "metadata_processor2".to_string() };
    
    let consumer1_node = graph.add_task(consumer1);
    let consumer2_node = graph.add_task(consumer2);
    
    // Create aggregator that depends on both consumers
    let aggregator = WorkflowTask::Aggregator { name: "final_result".to_string() };
    let aggregator_node = graph.add_task(aggregator);
    
    // Set up dependencies (from -> to)
    graph.add_dependency(producer1_node, consumer1_node)?;
    graph.add_dependency(producer2_node, consumer1_node)?;
    graph.add_dependency(producer3_node, consumer2_node)?;
    graph.add_dependency(consumer1_node, aggregator_node)?;
    graph.add_dependency(consumer2_node, aggregator_node)?;
    
    println!("Graph structure:");
    println!("- Producer1 & Producer2 → Consumer1");
    println!("- Producer3 → Consumer2"); 
    println!("- Consumer1 & Consumer2 → Aggregator");
    println!("- Total tasks: {}\n", graph.task_count());
    
    // Execute the graph
    let executor = DogeExecutor::new();
    let results = executor.execute(graph).await?;
    
    println!("\n=== Final Results ===");
    for (node_index, result) in results {
        match result {
            doge::TaskResult::Success(output) => {
                println!("Node {:?}: Success", node_index.index());
                // Print the output based on its type
                match output {
                    TaskOutput::ProducerData(data) => {
                        println!("  Producer Data: {:?}", data);
                    }
                    TaskOutput::ConsumerResult(result) => {
                        println!("  Consumer Result: {}", result);
                    }
                    TaskOutput::AggregateResult(result) => {
                        println!("  Aggregate Result: {}", result);
                    }
                }
            }
            doge::TaskResult::Error(err) => {
                println!("Node {:?}: Error - {:?}", node_index.index(), err);
            }
            doge::TaskResult::Cancelled => {
                println!("Node {:?}: Cancelled", node_index.index());
            }
        }
    }
    
    println!("\n✅ Successfully demonstrated dependency data flow!");
    println!("Tasks were able to access outputs from their dependencies via TaskContext");
    
    Ok(())
}