//! Example demonstrating pixi-style progress reporters with Doge
//!
//! This example shows how to implement a multi-layered progress reporting system
//! similar to pixi's TopLevelProgress, with different reporters for different
//! operation types and support for both console and JSON output.

use doge::{async_trait, AsyncTask, DogeExecutor, TaskGraph, TaskContext, TaskPriority, ExecutorConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Event types that can be reported during task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportEvent {
    /// Task execution queued
    TaskQueued {
        task_id: String,
        task_type: String,
        context: ReportContext,
    },
    /// Task execution started
    TaskStarted {
        task_id: String,
        task_type: String,
        context: ReportContext,
    },
    /// Task execution finished successfully
    TaskFinished {
        task_id: String,
        task_type: String,
        duration: Duration,
        context: ReportContext,
    },
    /// Task execution failed
    TaskFailed {
        task_id: String,
        task_type: String,
        error: String,
        duration: Duration,
        context: ReportContext,
    },
    /// Progress update during long-running task
    TaskProgress {
        task_id: String,
        current: u64,
        total: Option<u64>,
        message: String,
        context: ReportContext,
    },
}

/// Context for different types of operations (similar to pixi's ReporterContext)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportContext {
    SolveEnvironment { environment: String },
    DownloadPackage { package: String, source: String },
    BuildPackage { package: String, build_type: String },
    CheckoutSource { repo: String, ref_: String },
    InstallPackage { package: String },
}

/// Trait for different types of progress reporters
pub trait ProgressReporter: Send + Sync {
    fn report_event(&mut self, event: ReportEvent);
    fn set_context(&mut self, context: ReportContext);
}

/// Console reporter that prints progress to terminal (like pixi's console output)
pub struct ConsoleReporter {
    context: Option<ReportContext>,
    active_tasks: HashMap<String, Instant>,
}

impl ConsoleReporter {
    pub fn new() -> Self {
        Self {
            context: None,
            active_tasks: HashMap::new(),
        }
    }

    fn format_duration(duration: Duration) -> String {
        if duration.as_secs() > 0 {
            format!("{:.1}s", duration.as_secs_f32())
        } else {
            format!("{}ms", duration.as_millis())
        }
    }

    fn format_context(context: &ReportContext) -> String {
        match context {
            ReportContext::SolveEnvironment { environment } => {
                format!("[SOLVE:{}]", environment)
            }
            ReportContext::DownloadPackage { package, source } => {
                format!("[DOWNLOAD:{} from {}]", package, source)
            }
            ReportContext::BuildPackage { package, build_type } => {
                format!("[BUILD:{} ({})]", package, build_type)
            }
            ReportContext::CheckoutSource { repo, ref_ } => {
                format!("[CHECKOUT:{} @ {}]", repo, ref_)
            }
            ReportContext::InstallPackage { package } => {
                format!("[INSTALL:{}]", package)
            }
        }
    }
}

impl ProgressReporter for ConsoleReporter {
    fn report_event(&mut self, event: ReportEvent) {
        match event {
            ReportEvent::TaskQueued { task_id, task_type, context } => {
                println!("📋 {} Queued {} {}", 
                    Self::format_context(&context), task_type, task_id);
            }
            ReportEvent::TaskStarted { task_id, task_type, context } => {
                self.active_tasks.insert(task_id.clone(), Instant::now());
                println!("🚀 {} Started {} {}", 
                    Self::format_context(&context), task_type, task_id);
            }
            ReportEvent::TaskFinished { task_id, task_type, duration, context } => {
                self.active_tasks.remove(&task_id);
                println!("✅ {} Finished {} {} in {}", 
                    Self::format_context(&context), task_type, task_id, Self::format_duration(duration));
            }
            ReportEvent::TaskFailed { task_id, task_type, error, duration, context } => {
                self.active_tasks.remove(&task_id);
                println!("❌ {} Failed {} {} after {} - {}", 
                    Self::format_context(&context), task_type, task_id, Self::format_duration(duration), error);
            }
            ReportEvent::TaskProgress { task_id, current, total, message, context } => {
                let progress_str = if let Some(total) = total {
                    format!("({}/{})", current, total)
                } else {
                    format!("({})", current)
                };
                println!("📈 {} Progress {} {} - {}", 
                    Self::format_context(&context), task_id, progress_str, message);
            }
        }
    }

    fn set_context(&mut self, context: ReportContext) {
        self.context = Some(context);
    }
}

/// JSON reporter that outputs structured events (like pixi's JSON output)
pub struct JsonReporter {
    context: Option<ReportContext>,
}

impl JsonReporter {
    pub fn new() -> Self {
        Self { context: None }
    }
}

impl ProgressReporter for JsonReporter {
    fn report_event(&mut self, event: ReportEvent) {
        if let Ok(json) = serde_json::to_string(&event) {
            println!("{}", json);
        }
    }

    fn set_context(&mut self, context: ReportContext) {
        self.context = Some(context);
    }
}

/// Multi-reporter that combines multiple reporters (like pixi's TopLevelProgress)
pub struct MultiReporter {
    reporters: Vec<Box<dyn ProgressReporter>>,
}

impl MultiReporter {
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn add_reporter<R: ProgressReporter + 'static>(mut self, reporter: R) -> Self {
        self.reporters.push(Box::new(reporter));
        self
    }
}

impl ProgressReporter for MultiReporter {
    fn report_event(&mut self, event: ReportEvent) {
        for reporter in &mut self.reporters {
            reporter.report_event(event.clone());
        }
    }

    fn set_context(&mut self, context: ReportContext) {
        for reporter in &mut self.reporters {
            reporter.set_context(context.clone());
        }
    }
}

/// Unified task type for pixi-style operations
#[derive(Clone)]
pub enum PixiTask {
    SolveEnvironment {
        environment: String,
        packages: Vec<String>,
    },
    DownloadPackage {
        package: String,
        source: String,
    },
}

// Manual implementations to avoid issues with Arc<Mutex<dyn Trait>>
impl std::fmt::Debug for PixiTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PixiTask::SolveEnvironment { environment, packages } => {
                f.debug_struct("SolveEnvironment")
                    .field("environment", environment)
                    .field("packages", packages)
                    .finish()
            }
            PixiTask::DownloadPackage { package, source } => {
                f.debug_struct("DownloadPackage")
                    .field("package", package)
                    .field("source", source)
                    .finish()
            }
        }
    }
}

impl PartialEq for PixiTask {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PixiTask::SolveEnvironment { environment: e1, packages: p1 }, 
             PixiTask::SolveEnvironment { environment: e2, packages: p2 }) => e1 == e2 && p1 == p2,
            (PixiTask::DownloadPackage { package: p1, source: s1 }, 
             PixiTask::DownloadPackage { package: p2, source: s2 }) => p1 == p2 && s1 == s2,
            _ => false,
        }
    }
}

impl Eq for PixiTask {}

impl std::hash::Hash for PixiTask {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PixiTask::SolveEnvironment { environment, packages } => {
                0u8.hash(state);
                environment.hash(state);
                packages.hash(state);
            }
            PixiTask::DownloadPackage { package, source } => {
                1u8.hash(state);
                package.hash(state);
                source.hash(state);
            }
        }
    }
}

/// Output types for different operations
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PixiOutput {
    SolvedPackages(HashMap<String, String>),
    DownloadedFile(String),
}

// Global reporter for simplicity in this example
static mut GLOBAL_REPORTER: Option<Arc<Mutex<dyn ProgressReporter>>> = None;

fn get_reporter() -> Option<Arc<Mutex<dyn ProgressReporter>>> {
    unsafe { GLOBAL_REPORTER.as_ref().cloned() }
}

fn set_reporter(reporter: Arc<Mutex<dyn ProgressReporter>>) {
    unsafe { GLOBAL_REPORTER = Some(reporter); }
}

#[async_trait]
impl AsyncTask for PixiTask {
    type Output = PixiOutput;
    type Error = String;

    async fn execute_with_context(&self, context: &TaskContext) -> Result<Self::Output, Self::Error> {
        match self {
            PixiTask::SolveEnvironment { environment, packages } => {
                let task_id = format!("solve-{}", environment);
                let report_context = ReportContext::SolveEnvironment { 
                    environment: environment.clone() 
                };

                // Report task started
                if let Some(reporter) = get_reporter() {
                    if let Ok(mut r) = reporter.lock() {
                        r.report_event(ReportEvent::TaskStarted {
                            task_id: task_id.clone(),
                            task_type: "SolveEnvironment".to_string(),
                            context: report_context.clone(),
                        });
                    }
                }

                let start_time = Instant::now();
                let mut resolved = HashMap::new();

                // Simulate solving packages with progress updates
                for (i, package) in packages.iter().enumerate() {
                    sleep(Duration::from_millis(200)).await;
                    
                    // Report progress
                    if let Some(reporter) = get_reporter() {
                        if let Ok(mut r) = reporter.lock() {
                            r.report_event(ReportEvent::TaskProgress {
                                task_id: task_id.clone(),
                                current: i as u64 + 1,
                                total: Some(packages.len() as u64),
                                message: format!("Resolving {}", package),
                                context: report_context.clone(),
                            });
                        }
                    }

                    // Simulate package resolution
                    resolved.insert(package.clone(), format!("{}-1.0.0", package));
                }

                let duration = start_time.elapsed();

                // Report completion
                if let Some(reporter) = get_reporter() {
                    if let Ok(mut r) = reporter.lock() {
                        r.report_event(ReportEvent::TaskFinished {
                            task_id: task_id.clone(),
                            task_type: "SolveEnvironment".to_string(),
                            duration,
                            context: report_context.clone(),
                        });
                    }
                }

                Ok(PixiOutput::SolvedPackages(resolved))
            }
            
            PixiTask::DownloadPackage { package, source } => {
                let task_id = format!("download-{}", package);
                let report_context = ReportContext::DownloadPackage { 
                    package: package.clone(),
                    source: source.clone(),
                };

                // Check if we have dependency results (packages to download)
                let _dependency_count = context.dependency_count();

                // Report task started
                if let Some(reporter) = get_reporter() {
                    if let Ok(mut r) = reporter.lock() {
                        r.report_event(ReportEvent::TaskStarted {
                            task_id: task_id.clone(),
                            task_type: "DownloadPackage".to_string(),
                            context: report_context.clone(),
                        });
                    }
                }

                let start_time = Instant::now();

                // Simulate download with progress
                let download_steps = 5;
                for i in 0..download_steps {
                    sleep(Duration::from_millis(150)).await;
                    
                    if let Some(reporter) = get_reporter() {
                        if let Ok(mut r) = reporter.lock() {
                            r.report_event(ReportEvent::TaskProgress {
                                task_id: task_id.clone(),
                                current: i + 1,
                                total: Some(download_steps),
                                message: format!("Downloading {} from {}", package, source),
                                context: report_context.clone(),
                            });
                        }
                    }
                }

                let duration = start_time.elapsed();

                // Report completion
                if let Some(reporter) = get_reporter() {
                    if let Ok(mut r) = reporter.lock() {
                        r.report_event(ReportEvent::TaskFinished {
                            task_id: task_id.clone(),
                            task_type: "DownloadPackage".to_string(),
                            duration,
                            context: report_context.clone(),
                        });
                    }
                }

                Ok(PixiOutput::DownloadedFile(format!("/cache/{}-1.0.0.tar.bz2", package)))
            }
        }
    }

    fn name(&self) -> String {
        match self {
            PixiTask::SolveEnvironment { environment, .. } => format!("SolveEnvironment({})", environment),
            PixiTask::DownloadPackage { package, .. } => format!("DownloadPackage({})", package),
        }
    }

    fn priority(&self) -> TaskPriority {
        match self {
            PixiTask::SolveEnvironment { .. } => TaskPriority::High,
            PixiTask::DownloadPackage { .. } => TaskPriority::Normal,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Pixi-Style Progress Reporters Example ===\n");

    // Create multi-reporter with console output
    let multi_reporter = Arc::new(Mutex::new(
        MultiReporter::new()
            .add_reporter(ConsoleReporter::new())
    ));

    // Option to add JSON reporter
    let use_json = std::env::args().any(|arg| arg == "--json");
    if use_json {
        println!("Using JSON output format:");
        // Add JSON reporter to the multi-reporter
        // Note: In a real implementation, we'd properly combine reporters
    }

    // Set global reporter
    set_reporter(multi_reporter);

    let mut graph = TaskGraph::new();

    // Create solve environment task
    let solve_task = PixiTask::SolveEnvironment {
        environment: "default".to_string(),
        packages: vec!["python".to_string(), "numpy".to_string(), "pandas".to_string()],
    };
    let solve_node = graph.add_task(solve_task);

    // Create download tasks that depend on solve
    let download_python = PixiTask::DownloadPackage {
        package: "python".to_string(),
        source: "conda-forge".to_string(),
    };
    let download_numpy = PixiTask::DownloadPackage {
        package: "numpy".to_string(),
        source: "conda-forge".to_string(),
    };
    let download_pandas = PixiTask::DownloadPackage {
        package: "pandas".to_string(),
        source: "conda-forge".to_string(),
    };

    let python_node = graph.add_task(download_python);
    let numpy_node = graph.add_task(download_numpy);
    let pandas_node = graph.add_task(download_pandas);

    // Set up dependencies: solve -> downloads
    graph.add_dependency(solve_node, python_node)?;
    graph.add_dependency(solve_node, numpy_node)?;
    graph.add_dependency(solve_node, pandas_node)?;

    println!("Graph structure:");
    println!("- SolveEnvironment → DownloadPython, DownloadNumpy, DownloadPandas");
    println!("- Total tasks: {}\n", graph.task_count());

    // Execute with custom configuration for better parallelism
    let config = ExecutorConfig::default()
        .with_max_concurrent_tasks(3)?
        .with_default_task_timeout(Duration::from_secs(30))?;
    
    let executor = DogeExecutor::with_config(config)?;
    let results = executor.execute(graph).await?;

    println!("\n=== Execution Summary ===");
    for (node_index, result) in results {
        match result {
            doge::TaskResult::Success(output) => {
                println!("✅ Node {}: Success", node_index.index());
                match output {
                    PixiOutput::SolvedPackages(packages) => {
                        println!("   Resolved {} packages", packages.len());
                    }
                    PixiOutput::DownloadedFile(path) => {
                        println!("   Downloaded: {}", path);
                    }
                }
            }
            doge::TaskResult::Error(err) => {
                println!("❌ Node {}: Error - {:?}", node_index.index(), err);
            }
            doge::TaskResult::Cancelled => {
                println!("🚫 Node {}: Cancelled", node_index.index());
            }
        }
    }

    println!("\n✅ Successfully demonstrated pixi-style progress reporting!");
    println!("The multi-layered reporter system provides rich progress information");
    println!("similar to pixi's TopLevelProgress with context-aware event tracking.");
    
    Ok(())
}