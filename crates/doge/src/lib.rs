//! Doge - Async DAG Task Executor
//!
//! A high-performance, async task execution engine built on top of the `daggy` crate
//! for managing directed acyclic graphs (DAGs) of interdependent tasks.
//!
//! # Overview
//!
//! Doge provides a task scheduling and execution framework where:
//! - Each task is an asynchronous function that can be scheduled by the tokio runtime
//! - Tasks are organized in a DAG to represent dependencies
//! - Duplicate tasks are automatically deduplicated
//! - Execution is optimized for parallelism while respecting dependencies
//!
//! # Example
//!
//! ```rust
//! use doge::{AsyncTask, TaskGraph, DogeExecutor};
//! use std::sync::Arc;
//!
//! #[derive(Clone, Debug, PartialEq, Eq, Hash)]
//! struct AddTask {
//!     a: i32,
//!     b: i32,
//! }
//!
//! #[async_trait::async_trait]
//! impl AsyncTask for AddTask {
//!     type Output = i32;
//!     type Error = String;
//!
//!     async fn execute(&self) -> Result<Self::Output, Self::Error> {
//!         Ok(self.a + self.b)
//!     }
//!
//!     fn name(&self) -> String {
//!         format!("add({}, {})", self.a, self.b)
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut graph = TaskGraph::new();
//!     let task1 = AddTask { a: 1, b: 2 };
//!     let task2 = AddTask { a: 3, b: 4 };
//!     
//!     let node1 = graph.add_task(task1);
//!     let node2 = graph.add_task(task2);
//!     
//!     let executor = DogeExecutor::new();
//!     let results = executor.execute(graph).await?;
//!     
//!     println!("Results: {:?}", results);
//!     Ok(())
//! }
//! ```

pub mod context;
pub mod error;
pub mod executor;
pub mod graph;
pub mod reporter;
pub mod task;
pub mod types;

pub use context::TaskContext;
pub use error::{DogeError, ExecutionError};
pub use executor::DogeExecutor;
pub use graph::{TaskGraph, TaskNode, NodeIndex};
pub use reporter::{ExecutionReporter, TaskExecutionEvent, SimpleConsoleReporter, MultiReporter, NoOpReporter};
pub use task::{AsyncTask, TaskId, TaskState, TaskResult, TaskPriority};
pub use types::ExecutorConfig;

use std::future::Future;
use std::pin::Pin;

/// A boxed future that is Send and can be used across thread boundaries
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// A boxed future that is not Send and must be used on a single thread
pub type LocalBoxFuture<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

// Re-export async_trait for convenience
pub use async_trait::async_trait;