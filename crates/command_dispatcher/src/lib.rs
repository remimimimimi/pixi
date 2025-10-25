//! Core task orchestration primitives extracted from the Pixi command dispatcher.
//! This crate intentionally avoids Pixi-specific types so it can be reused by
//! other projects that need a deduplicating background executor.

pub mod executor;
pub mod limits;
pub mod task;

pub use executor::{Executor, ExecutorFutures};
pub use limits::{Limit, Limits, ResolvedLimits};
pub use task::{TaskFuture, TaskReason, TaskSpec};
