//! Progress reporting system for Doge task execution
//!
//! This module provides a reporter trait and implementation that allows
//! integration with external progress reporting systems like pixi's 
//! TopLevelProgress system.

use std::time::Duration;
use crate::graph::NodeIndex;
use crate::task::TaskId;

/// Event types that can be reported during task execution
#[derive(Debug, Clone)]
pub enum TaskExecutionEvent {
    /// Task was queued for execution
    TaskQueued {
        node_index: NodeIndex,
        task_id: TaskId,
        task_name: String,
    },
    /// Task execution started
    TaskStarted {
        node_index: NodeIndex,
        task_id: TaskId,
        task_name: String,
    },
    /// Task execution completed successfully
    TaskCompleted {
        node_index: NodeIndex,
        task_id: TaskId,
        task_name: String,
        duration: Duration,
    },
    /// Task execution failed
    TaskFailed {
        node_index: NodeIndex,
        task_id: TaskId,
        task_name: String,
        error: String,
        duration: Duration,
    },
    /// Task was retried after failure
    TaskRetried {
        node_index: NodeIndex,
        task_id: TaskId,
        task_name: String,
        attempt: u32,
    },
    /// Progress update during long-running task
    TaskProgress {
        node_index: NodeIndex,
        task_id: TaskId,
        current: u64,
        total: Option<u64>,
        message: String,
    },
    /// Execution batch started
    ExecutionStarted {
        total_tasks: usize,
    },
    /// Execution batch completed
    ExecutionCompleted {
        total_tasks: usize,
        successful_tasks: usize,
        failed_tasks: usize,
        duration: Duration,
    },
}

/// Trait for reporting task execution progress
///
/// This trait can be implemented to integrate with external progress
/// reporting systems like pixi's TopLevelProgress.
pub trait ExecutionReporter: Send + Sync {
    /// Report a task execution event
    fn report_event(&mut self, event: TaskExecutionEvent);
    
    /// Set context for the current execution batch
    fn set_context(&mut self, context: String) {
        let _ = context; // Default implementation ignores context
    }
    
    /// Called when the reporter is no longer needed
    fn finish(&mut self) {
        // Default implementation does nothing
    }
}

/// A no-op reporter that discards all events
#[derive(Debug, Default)]
pub struct NoOpReporter;

impl ExecutionReporter for NoOpReporter {
    fn report_event(&mut self, _event: TaskExecutionEvent) {
        // Do nothing
    }
}

/// A simple console reporter for debugging
#[derive(Debug)]
pub struct SimpleConsoleReporter {
    context: Option<String>,
}

impl SimpleConsoleReporter {
    pub fn new() -> Self {
        Self { context: None }
    }
    
    fn format_duration(duration: Duration) -> String {
        if duration.as_secs() > 0 {
            format!("{:.1}s", duration.as_secs_f32())
        } else {
            format!("{}ms", duration.as_millis())
        }
    }
}

impl Default for SimpleConsoleReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionReporter for SimpleConsoleReporter {
    fn report_event(&mut self, event: TaskExecutionEvent) {
        let context_prefix = self.context
            .as_ref()
            .map(|c| format!("[{}] ", c))
            .unwrap_or_default();
            
        match event {
            TaskExecutionEvent::TaskQueued { task_name, .. } => {
                println!("📋 {}Queued {}", context_prefix, task_name);
            }
            TaskExecutionEvent::TaskStarted { task_name, .. } => {
                println!("🚀 {}Started {}", context_prefix, task_name);
            }
            TaskExecutionEvent::TaskCompleted { task_name, duration, .. } => {
                println!("✅ {}Completed {} in {}", 
                    context_prefix, task_name, Self::format_duration(duration));
            }
            TaskExecutionEvent::TaskFailed { task_name, error, duration, .. } => {
                println!("❌ {}Failed {} after {} - {}", 
                    context_prefix, task_name, Self::format_duration(duration), error);
            }
            TaskExecutionEvent::TaskRetried { task_name, attempt, .. } => {
                println!("🔄 {}Retrying {} (attempt {})", 
                    context_prefix, task_name, attempt);
            }
            TaskExecutionEvent::TaskProgress { current, total, message, .. } => {
                let progress = if let Some(total) = total {
                    format!("({}/{})", current, total)
                } else {
                    format!("({})", current)
                };
                println!("📈 {}Progress {} - {}", 
                    context_prefix, progress, message);
            }
            TaskExecutionEvent::ExecutionStarted { total_tasks } => {
                println!("🎯 {}Starting execution of {} tasks", context_prefix, total_tasks);
            }
            TaskExecutionEvent::ExecutionCompleted { 
                total_tasks, 
                successful_tasks, 
                failed_tasks, 
                duration 
            } => {
                println!("🏁 {}Execution completed: {}/{} successful, {} failed in {}", 
                    context_prefix, successful_tasks, total_tasks, failed_tasks, 
                    Self::format_duration(duration));
            }
        }
    }
    
    fn set_context(&mut self, context: String) {
        self.context = Some(context);
    }
}

/// A multi-reporter that broadcasts events to multiple reporters
pub struct MultiReporter {
    reporters: Vec<Box<dyn ExecutionReporter>>,
}

impl MultiReporter {
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }
    
    pub fn add_reporter<R: ExecutionReporter + 'static>(mut self, reporter: R) -> Self {
        self.reporters.push(Box::new(reporter));
        self
    }
    
    pub fn with_reporter<R: ExecutionReporter + 'static>(&mut self, reporter: R) {
        self.reporters.push(Box::new(reporter));
    }
}

impl Default for MultiReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionReporter for MultiReporter {
    fn report_event(&mut self, event: TaskExecutionEvent) {
        for reporter in &mut self.reporters {
            reporter.report_event(event.clone());
        }
    }
    
    fn set_context(&mut self, context: String) {
        for reporter in &mut self.reporters {
            reporter.set_context(context.clone());
        }
    }
    
    fn finish(&mut self) {
        for reporter in &mut self.reporters {
            reporter.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    
    #[derive(Debug, Default)]
    struct TestReporter {
        events: Arc<Mutex<Vec<TaskExecutionEvent>>>,
    }
    
    impl TestReporter {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }
        
        fn get_events(&self) -> Vec<TaskExecutionEvent> {
            self.events.lock().unwrap().clone()
        }
    }
    
    impl ExecutionReporter for TestReporter {
        fn report_event(&mut self, event: TaskExecutionEvent) {
            self.events.lock().unwrap().push(event);
        }
    }
    
    #[test]
    fn test_simple_console_reporter() {
        let mut reporter = SimpleConsoleReporter::new();
        reporter.set_context("test".to_string());
        
        // This would print to console, but we can't easily test output
        // Instead we just verify it doesn't panic
        reporter.report_event(TaskExecutionEvent::TaskStarted {
            node_index: crate::graph::NodeIndex::new(0),
            task_id: TaskId::new(),
            task_name: "test_task".to_string(),
        });
    }
    
    #[test]
    fn test_multi_reporter() {
        let reporter1 = TestReporter::new();
        let reporter2 = TestReporter::new();
        
        let events1 = reporter1.events.clone();
        let events2 = reporter2.events.clone();
        
        let mut multi = MultiReporter::new()
            .add_reporter(reporter1)
            .add_reporter(reporter2);
            
        let event = TaskExecutionEvent::TaskStarted {
            node_index: crate::graph::NodeIndex::new(0),
            task_id: TaskId::new(),
            task_name: "test_task".to_string(),
        };
        
        multi.report_event(event);
        
        // Both reporters should have received the event
        assert_eq!(events1.lock().unwrap().len(), 1);
        assert_eq!(events2.lock().unwrap().len(), 1);
    }
    
    #[test]
    fn test_no_op_reporter() {
        let mut reporter = NoOpReporter;
        
        // Should not panic
        reporter.report_event(TaskExecutionEvent::TaskStarted {
            node_index: crate::graph::NodeIndex::new(0),
            task_id: TaskId::new(),
            task_name: "test_task".to_string(),
        });
        
        reporter.set_context("test".to_string());
        reporter.finish();
    }
}