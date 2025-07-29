# Doge - Async DAG Task Executor

A high-performance, async task execution engine built on top of the `daggy` crate (which uses `petgraph`) for managing directed acyclic graphs (DAGs) of interdependent tasks. This crate is designed to replace the core functionality of `pixi_command_dispatcher` with a more generic, reusable, and tokio-native approach.

## Overview

Doge provides a task scheduling and execution framework where:
- Each task is an asynchronous function that can be scheduled by the tokio runtime
- Tasks are organized in a DAG to represent dependencies
- Duplicate tasks are automatically deduplicated
- Execution is optimized for parallelism while respecting dependencies
- Built-in caching and error handling

## Core Features

### 1. DAG-Based Task Management
- **Dependency Resolution**: Tasks are organized in a directed acyclic graph where edges represent dependencies
- **Cycle Detection**: Automatic prevention of circular dependencies using daggy's built-in cycle detection
- **Topological Scheduling**: Tasks are scheduled according to topological order, ensuring dependencies are satisfied

### 2. Async Task Execution
- **Tokio Integration**: Full integration with tokio runtime for async task execution
- **Concurrent Execution**: Tasks without dependencies can run in parallel
- **Resource Management**: Configurable limits on concurrent task execution

### 3. Task Deduplication
- **Content-Based Deduplication**: Multiple requests for the same task are automatically merged
- **Result Sharing**: When a deduplicated task completes, all waiting requests receive the same result
- **Efficient Resource Usage**: Prevents redundant computation and resource usage

### 4. Error Handling & Recovery
- **Graceful Error Propagation**: Failed tasks propagate errors to dependent tasks
- **Retry Mechanisms**: Configurable retry policies for transient failures
- **Error Context**: Rich error information with task dependency chains

### 5. Progress Reporting
- **Task Progress**: Real-time visibility into task execution status
- **Dependency Visualization**: Tools for understanding task relationships
- **Performance Metrics**: Built-in instrumentation for monitoring and debugging

## Comparison with Current pixi_command_dispatcher

| Feature | pixi_command_dispatcher | Doge |
|---------|------------------------|------|
| **Task Model** | Message-passing with background thread | Native async functions with tokio |
| **Dependency Management** | Manual coordination via HashMap | Explicit DAG with daggy/petgraph |
| **Concurrency** | Custom executor with manual future management | Tokio-native scheduling |
| **Deduplication** | Hash-based task deduplication | Content-based with DAG node sharing |
| **Error Handling** | oneshot channels with custom error types | Standard Result types with rich context |
| **Memory Usage** | Higher due to message passing overhead | Lower with direct task execution |
| **Debugging** | Difficult to trace task dependencies | Visual DAG inspection and traversal tools |
| **Testing** | Complex due to background thread coordination | Easier with deterministic async execution |
| **Extensibility** | Tightly coupled to pixi-specific types | Generic task executor for any async function |

## Development Plan

### Phase 1: Core DAG Framework
- [ ] **Task Specification API** - Define trait for async tasks with inputs/outputs
- [ ] **DAG Builder** - API for constructing task dependency graphs
- [ ] **Basic Executor** - Simple tokio-based execution engine
- [ ] **Deduplication Logic** - Content-based task deduplication
- [ ] **Error Types** - Rich error handling with dependency context

### Phase 2: Advanced Features  
- [ ] **Resource Limits** - Configurable concurrency and resource constraints
- [ ] **Progress Reporting** - Real-time task status and progress tracking
- [ ] **Caching Layer** - Persistent and in-memory result caching
- [ ] **Retry Policies** - Configurable retry mechanisms for failed tasks
- [ ] **Cancellation Support** - Graceful task cancellation and cleanup

### Phase 3: Performance & Optimization
- [ ] **Memory Optimization** - Efficient memory usage for large DAGs
- [ ] **Task Prioritization** - Priority-based scheduling within dependency constraints
- [ ] **Batch Operations** - Efficient handling of multiple related tasks
- [ ] **Instrumentation** - Performance metrics and debugging tools
- [ ] **Benchmarking** - Performance comparison with current implementation

### Phase 4: Integration & Migration
- [ ] **Adapter Layer** - Compatibility layer for existing pixi_command_dispatcher APIs
- [ ] **Migration Tools** - Utilities for converting existing task specifications
- [ ] **Integration Tests** - Comprehensive testing with real pixi workloads  
- [ ] **Documentation** - Complete API documentation and migration guide
- [ ] **Performance Validation** - Ensure performance parity or improvement

## Architecture Overview

```rust
// Core abstractions
trait AsyncTask {
    type Input;
    type Output;
    type Error;
    
    async fn execute(input: Self::Input) -> Result<Self::Output, Self::Error>;
}

// DAG builder for task dependencies
struct TaskGraphBuilder<T> {
    dag: daggy::Dag<TaskNode<T>, ()>,
    // Task deduplication and dependency tracking
}

// Main executor
struct DogeExecutor {
    runtime: tokio::runtime::Handle,
    // Resource limits, caching, progress reporting
}
```

## Key Benefits

1. **Better Performance**: Native tokio scheduling eliminates message-passing overhead
2. **Improved Debuggability**: Visual DAG representation makes dependency issues obvious
3. **Enhanced Testability**: Deterministic async execution simplifies testing
4. **Greater Flexibility**: Generic design allows reuse beyond pixi
5. **Cleaner Architecture**: Explicit dependency modeling vs implicit coordination
6. **Resource Efficiency**: Better memory usage and CPU utilization

## Migration Strategy

The migration from pixi_command_dispatcher to Doge will be gradual:

1. **Parallel Development**: Build Doge alongside existing dispatcher
2. **Adapter Pattern**: Create compatibility layer for existing APIs
3. **Incremental Migration**: Move task types one by one to new system
4. **Performance Validation**: Ensure each migrated component performs as well or better
5. **Complete Replacement**: Remove old dispatcher once all functionality is migrated

This approach ensures zero downtime and allows for easy rollback if issues are discovered during migration.