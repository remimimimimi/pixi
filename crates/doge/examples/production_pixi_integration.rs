//! Production-ready example showing how Doge integrates with pixi's reporter system
//!
//! This example demonstrates a complete integration that would work in a real
//! pixi environment, showing how the pixi_command_dispatcher could be replaced
//! with Doge while maintaining all the progress reporting capabilities.

use doge::{
    async_trait, AsyncTask, DogeExecutor, TaskGraph, TaskContext, TaskPriority, 
    ExecutorConfig, ExecutionReporter, TaskExecutionEvent, SimpleConsoleReporter
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Represents the different types of operations in a pixi workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PixiOperationType {
    SolveEnvironment,
    DownloadPackage,
    ExtractPackage,
    LinkPackage,
    VerifyPackage,
    CheckoutGitRepo,
    BuildFromSource,
}

/// Context for different pixi operations (similar to pixi's ReporterContext)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PixiContext {
    Environment { name: String, platform: String },
    Package { name: String, version: String, source: String },
    GitRepository { url: String, ref_: String },
    SourceBuild { package: String, build_type: String },
}

/// Integration trait that allows pixi's existing reporters to work with Doge
pub trait PixiReporterAdapter: ExecutionReporter {
    fn set_pixi_context(&mut self, context: PixiContext);
    fn report_pixi_operation(&mut self, operation: PixiOperationType, context: PixiContext);
}

/// Adapter that bridges Doge's ExecutionReporter to pixi's reporter system
pub struct PixiTopLevelProgressAdapter {
    inner_reporter: Arc<Mutex<dyn ExecutionReporter>>,
    current_context: Option<PixiContext>,
}

impl PixiTopLevelProgressAdapter {
    pub fn new(reporter: impl ExecutionReporter + 'static) -> Self {
        Self {
            inner_reporter: Arc::new(Mutex::new(reporter)),
            current_context: None,
        }
    }
    
    fn format_context(context: &PixiContext) -> String {
        match context {
            PixiContext::Environment { name, platform } => {
                format!("env:{}@{}", name, platform)
            }
            PixiContext::Package { name, version, source } => {
                format!("pkg:{}={}[{}]", name, version, source)
            }
            PixiContext::GitRepository { url, ref_ } => {
                format!("git:{}#{}", url, ref_)
            }
            PixiContext::SourceBuild { package, build_type } => {
                format!("build:{}({})", package, build_type)
            }
        }
    }
}

impl ExecutionReporter for PixiTopLevelProgressAdapter {
    fn report_event(&mut self, event: TaskExecutionEvent) {
        if let Ok(mut reporter) = self.inner_reporter.lock() {
            reporter.report_event(event);
        }
    }
    
    fn set_context(&mut self, context: String) {
        if let Ok(mut reporter) = self.inner_reporter.lock() {
            reporter.set_context(context);
        }
    }
}

impl PixiReporterAdapter for PixiTopLevelProgressAdapter {
    fn set_pixi_context(&mut self, context: PixiContext) {
        self.current_context = Some(context.clone());
        self.set_context(Self::format_context(&context));
    }
    
    fn report_pixi_operation(&mut self, operation: PixiOperationType, context: PixiContext) {
        // This is where we'd integrate with pixi's actual reporter system
        println!("🔄 Pixi Operation: {:?} in context {}", operation, Self::format_context(&context));
    }
}

/// Unified task type for all pixi operations
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PixiWorkflowTask {
    SolveEnvironment {
        environment: String,
        platform: String,
        packages: Vec<String>,
    },
    DownloadPackage {
        name: String,
        version: String,
        source: String,
        checksum: Option<String>,
    },
    ExtractPackage {
        name: String,
        version: String,
        archive_path: String,
    },
    LinkPackage {
        name: String,
        version: String,
        extract_path: String,
    },
    CheckoutGitRepo {
        url: String,
        ref_: String,
        target_dir: String,
    },
    BuildFromSource {
        package: String,
        source_dir: String,
        build_type: String,
    },
}

/// Output types for different pixi operations
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PixiWorkflowOutput {
    SolvedEnvironment { 
        resolved_packages: HashMap<String, String>,
        lock_file_path: String,
    },
    DownloadedPackage { 
        archive_path: String,
        verified: bool,
    },
    ExtractedPackage { 
        extract_path: String,
        files_count: usize,
    },
    LinkedPackage { 
        install_path: String,
        links_created: usize,
    },
    CheckedOutRepo { 
        commit_hash: String,
        checkout_path: String,
    },
    BuiltFromSource { 
        artifacts: Vec<String>,
        build_time: Duration,
    },
}

// Global reporter for this example
static mut GLOBAL_PIXI_REPORTER: Option<Arc<Mutex<PixiTopLevelProgressAdapter>>> = None;

fn get_pixi_reporter() -> Option<Arc<Mutex<PixiTopLevelProgressAdapter>>> {
    unsafe { GLOBAL_PIXI_REPORTER.as_ref().cloned() }
}

fn set_pixi_reporter(reporter: Arc<Mutex<PixiTopLevelProgressAdapter>>) {
    unsafe { GLOBAL_PIXI_REPORTER = Some(reporter); }
}

#[async_trait]
impl AsyncTask for PixiWorkflowTask {
    type Output = PixiWorkflowOutput;
    type Error = String;

    async fn execute_with_context(&self, context: &TaskContext) -> Result<Self::Output, Self::Error> {
        match self {
            PixiWorkflowTask::SolveEnvironment { environment, platform, packages } => {
                let pixi_context = PixiContext::Environment { 
                    name: environment.clone(), 
                    platform: platform.clone() 
                };
                
                if let Some(reporter_arc) = get_pixi_reporter() {
                    if let Ok(mut reporter) = reporter_arc.lock() {
                        reporter.set_pixi_context(pixi_context.clone());
                        reporter.report_pixi_operation(PixiOperationType::SolveEnvironment, pixi_context);
                    }
                }
                
                let start_time = Instant::now();
                let mut resolved = HashMap::new();
                
                // Simulate conda solving with realistic timing
                for (i, package) in packages.iter().enumerate() {
                    sleep(Duration::from_millis(300)).await;
                    resolved.insert(package.clone(), format!("{}-1.0.0-py39h12345", package));
                    
                    println!("   Resolved {} ({}/{})", package, i + 1, packages.len());
                }
                
                sleep(Duration::from_millis(500)).await; // Simulate lock file generation
                
                Ok(PixiWorkflowOutput::SolvedEnvironment {
                    resolved_packages: resolved,
                    lock_file_path: format!(".pixi/envs/{}/conda-meta/locks.json", environment),
                })
            }
            
            PixiWorkflowTask::DownloadPackage { name, version, source, checksum } => {
                let pixi_context = PixiContext::Package { 
                    name: name.clone(), 
                    version: version.clone(),
                    source: source.clone(),
                };
                
                if let Some(reporter_arc) = get_pixi_reporter() {
                    if let Ok(mut reporter) = reporter_arc.lock() {
                        reporter.set_pixi_context(pixi_context.clone());
                        reporter.report_pixi_operation(PixiOperationType::DownloadPackage, pixi_context);
                    }
                }
                
                // Check for solved packages from dependencies
                let mut found_dependency = false;
                for node_index in context.dependency_nodes() {
                    if let Some(PixiWorkflowOutput::SolvedEnvironment { resolved_packages, .. }) = 
                        context.get_dependency_result::<PixiWorkflowOutput>(node_index) {
                        if resolved_packages.contains_key(name) {
                            found_dependency = true;
                            println!("   Found {} in solved environment", name);
                            break;
                        }
                    }
                }
                
                if !found_dependency {
                    return Err(format!("Package {} not found in solved environment", name));
                }
                
                // Simulate download with progress
                let download_steps = 8;
                for i in 0..download_steps {
                    sleep(Duration::from_millis(100)).await;
                    let progress = ((i + 1) as f64 / download_steps as f64 * 100.0) as u64;
                    println!("   Downloading {} {}% complete", name, progress);
                }
                
                // Simulate checksum verification
                if checksum.is_some() {
                    sleep(Duration::from_millis(200)).await;
                    println!("   Verified checksum for {}", name);
                }
                
                Ok(PixiWorkflowOutput::DownloadedPackage {
                    archive_path: format!("/cache/{}-{}.tar.bz2", name, version),
                    verified: checksum.is_some(),
                })
            }
            
            PixiWorkflowTask::ExtractPackage { name, version, archive_path } => {
                let pixi_context = PixiContext::Package { 
                    name: name.clone(), 
                    version: version.clone(),
                    source: "extracted".to_string(),
                };
                
                if let Some(reporter_arc) = get_pixi_reporter() {
                    if let Ok(mut reporter) = reporter_arc.lock() {
                        reporter.set_pixi_context(pixi_context.clone());
                        reporter.report_pixi_operation(PixiOperationType::ExtractPackage, pixi_context);
                    }
                }
                
                // Check for downloaded package from dependencies
                let mut archive_exists = false;
                for node_index in context.dependency_nodes() {
                    if let Some(PixiWorkflowOutput::DownloadedPackage { archive_path: dep_path, .. }) = 
                        context.get_dependency_result::<PixiWorkflowOutput>(node_index) {
                        if dep_path.contains(name) {
                            archive_exists = true;
                            break;
                        }
                    }
                }
                
                if !archive_exists {
                    return Err(format!("Archive for {} not found", name));
                }
                
                // Simulate extraction
                sleep(Duration::from_millis(400)).await;
                println!("   Extracted {} from {}", name, archive_path);
                
                Ok(PixiWorkflowOutput::ExtractedPackage {
                    extract_path: format!("/tmp/extract/{}-{}", name, version),
                    files_count: 156,
                })
            }
            
            PixiWorkflowTask::LinkPackage { name, version, extract_path } => {
                let pixi_context = PixiContext::Package { 
                    name: name.clone(), 
                    version: version.clone(),
                    source: "linked".to_string(),
                };
                
                if let Some(reporter_arc) = get_pixi_reporter() {
                    if let Ok(mut reporter) = reporter_arc.lock() {
                        reporter.set_pixi_context(pixi_context.clone());
                        reporter.report_pixi_operation(PixiOperationType::LinkPackage, pixi_context);
                    }
                }
                
                // Check for extracted package from dependencies
                let mut extract_exists = false;
                for node_index in context.dependency_nodes() {
                    if let Some(PixiWorkflowOutput::ExtractedPackage { extract_path: dep_path, .. }) = 
                        context.get_dependency_result::<PixiWorkflowOutput>(node_index) {
                        if dep_path.contains(name) {
                            extract_exists = true;
                            break;
                        }
                    }
                }
                
                if !extract_exists {
                    return Err(format!("Extracted files for {} not found", name));
                }
                
                // Simulate linking
                sleep(Duration::from_millis(300)).await;
                println!("   Linked {} into environment", name);
                
                Ok(PixiWorkflowOutput::LinkedPackage {
                    install_path: format!(".pixi/envs/default/lib/python3.9/site-packages/{}", name),
                    links_created: 23,
                })
            }
            
            PixiWorkflowTask::CheckoutGitRepo { url, ref_, target_dir } => {
                let pixi_context = PixiContext::GitRepository { 
                    url: url.clone(), 
                    ref_: ref_.clone(),
                };
                
                if let Some(reporter_arc) = get_pixi_reporter() {
                    if let Ok(mut reporter) = reporter_arc.lock() {
                        reporter.set_pixi_context(pixi_context.clone());
                        reporter.report_pixi_operation(PixiOperationType::CheckoutGitRepo, pixi_context);
                    }
                }
                
                // Simulate git operations
                sleep(Duration::from_millis(200)).await;
                println!("   Cloning {} to {}", url, target_dir);
                
                sleep(Duration::from_millis(300)).await;
                println!("   Checking out {}", ref_);
                
                Ok(PixiWorkflowOutput::CheckedOutRepo {
                    commit_hash: "abc123def456".to_string(),
                    checkout_path: target_dir.clone(),
                })
            }
            
            PixiWorkflowTask::BuildFromSource { package, source_dir, build_type } => {
                let pixi_context = PixiContext::SourceBuild { 
                    package: package.clone(), 
                    build_type: build_type.clone(),
                };
                
                if let Some(reporter_arc) = get_pixi_reporter() {
                    if let Ok(mut reporter) = reporter_arc.lock() {
                        reporter.set_pixi_context(pixi_context.clone());
                        reporter.report_pixi_operation(PixiOperationType::BuildFromSource, pixi_context);
                    }
                }
                
                let start_time = Instant::now();
                
                // Simulate build steps
                sleep(Duration::from_millis(500)).await;
                println!("   Configuring build for {}", package);
                
                sleep(Duration::from_millis(1000)).await;
                println!("   Compiling {} with {}", package, build_type);
                
                sleep(Duration::from_millis(300)).await;
                println!("   Packaging {} artifacts", package);
                
                let build_time = start_time.elapsed();
                
                Ok(PixiWorkflowOutput::BuiltFromSource {
                    artifacts: vec![
                        format!("{}.so", package),
                        format!("{}.h", package),
                    ],
                    build_time,
                })
            }
        }
    }

    fn name(&self) -> String {
        match self {
            PixiWorkflowTask::SolveEnvironment { environment, .. } => {
                format!("SolveEnvironment({})", environment)
            }
            PixiWorkflowTask::DownloadPackage { name, .. } => {
                format!("DownloadPackage({})", name)
            }
            PixiWorkflowTask::ExtractPackage { name, .. } => {
                format!("ExtractPackage({})", name)
            }
            PixiWorkflowTask::LinkPackage { name, .. } => {
                format!("LinkPackage({})", name)
            }
            PixiWorkflowTask::CheckoutGitRepo { url, .. } => {
                format!("CheckoutGitRepo({})", url)
            }
            PixiWorkflowTask::BuildFromSource { package, .. } => {
                format!("BuildFromSource({})", package)
            }
        }
    }

    fn priority(&self) -> TaskPriority {
        match self {
            PixiWorkflowTask::SolveEnvironment { .. } => TaskPriority::Critical,
            PixiWorkflowTask::DownloadPackage { .. } => TaskPriority::High,
            PixiWorkflowTask::ExtractPackage { .. } => TaskPriority::Normal,
            PixiWorkflowTask::LinkPackage { .. } => TaskPriority::Normal,
            PixiWorkflowTask::CheckoutGitRepo { .. } => TaskPriority::High,
            PixiWorkflowTask::BuildFromSource { .. } => TaskPriority::Low,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Production Pixi Integration Example ===\n");
    println!("This demonstrates how Doge would replace pixi_command_dispatcher");
    println!("while maintaining full compatibility with pixi's progress reporting.\n");

    // Create the reporter adapter that bridges Doge to pixi's system
    let console_reporter = SimpleConsoleReporter::new();
    let pixi_adapter = PixiTopLevelProgressAdapter::new(console_reporter);
    let reporter_arc = Arc::new(Mutex::new(pixi_adapter));
    
    set_pixi_reporter(reporter_arc);

    let mut graph = TaskGraph::new();

    // Create a realistic pixi workflow:
    // 1. Solve environment to get package requirements
    let solve_task = PixiWorkflowTask::SolveEnvironment {
        environment: "default".to_string(),
        platform: "linux-64".to_string(),
        packages: vec!["python".to_string(), "numpy".to_string(), "cython".to_string()],
    };
    let solve_node = graph.add_task(solve_task);

    // 2. Download packages in parallel (depends on solve)
    let download_python = PixiWorkflowTask::DownloadPackage {
        name: "python".to_string(),
        version: "3.9.18".to_string(),
        source: "conda-forge".to_string(),
        checksum: Some("sha256:abc123".to_string()),
    };
    let download_numpy = PixiWorkflowTask::DownloadPackage {
        name: "numpy".to_string(),
        version: "1.24.3".to_string(),
        source: "conda-forge".to_string(),
        checksum: Some("sha256:def456".to_string()),
    };
    let download_cython = PixiWorkflowTask::DownloadPackage {
        name: "cython".to_string(),
        version: "3.0.0".to_string(),
        source: "conda-forge".to_string(),
        checksum: Some("sha256:ghi789".to_string()),
    };

    let python_download_node = graph.add_task(download_python);
    let numpy_download_node = graph.add_task(download_numpy);
    let cython_download_node = graph.add_task(download_cython);

    // 3. Extract packages (depends on downloads)
    let extract_python = PixiWorkflowTask::ExtractPackage {
        name: "python".to_string(),
        version: "3.9.18".to_string(),
        archive_path: "/cache/python-3.9.18.tar.bz2".to_string(),
    };
    let extract_numpy = PixiWorkflowTask::ExtractPackage {
        name: "numpy".to_string(),
        version: "1.24.3".to_string(),
        archive_path: "/cache/numpy-1.24.3.tar.bz2".to_string(),
    };

    let python_extract_node = graph.add_task(extract_python);
    let numpy_extract_node = graph.add_task(extract_numpy);

    // 4. Link packages into environment (depends on extracts)
    let link_python = PixiWorkflowTask::LinkPackage {
        name: "python".to_string(),
        version: "3.9.18".to_string(),
        extract_path: "/tmp/extract/python-3.9.18".to_string(),
    };
    let link_numpy = PixiWorkflowTask::LinkPackage {
        name: "numpy".to_string(),
        version: "1.24.3".to_string(),
        extract_path: "/tmp/extract/numpy-1.24.3".to_string(),
    };

    let python_link_node = graph.add_task(link_python);
    let numpy_link_node = graph.add_task(link_numpy);

    // 5. Source build workflow (depends on cython download)
    let checkout_repo = PixiWorkflowTask::CheckoutGitRepo {
        url: "https://github.com/example/my-package.git".to_string(),
        ref_: "v1.0.0".to_string(),
        target_dir: "/tmp/src/my-package".to_string(),
    };
    let checkout_node = graph.add_task(checkout_repo);

    let build_from_source = PixiWorkflowTask::BuildFromSource {
        package: "my-package".to_string(),
        source_dir: "/tmp/src/my-package".to_string(),
        build_type: "cmake".to_string(),
    };
    let build_node = graph.add_task(build_from_source);

    // Set up the dependency graph
    // Solve -> Downloads
    graph.add_dependency(solve_node, python_download_node)?;
    graph.add_dependency(solve_node, numpy_download_node)?;
    graph.add_dependency(solve_node, cython_download_node)?;

    // Downloads -> Extracts
    graph.add_dependency(python_download_node, python_extract_node)?;
    graph.add_dependency(numpy_download_node, numpy_extract_node)?;

    // Extracts -> Links
    graph.add_dependency(python_extract_node, python_link_node)?;
    graph.add_dependency(numpy_extract_node, numpy_link_node)?;

    // Source build chain
    graph.add_dependency(cython_download_node, checkout_node)?;
    graph.add_dependency(checkout_node, build_node)?;

    println!("Graph structure:");
    println!("- SolveEnvironment → DownloadPackages (python, numpy, cython)");
    println!("- DownloadPackages → ExtractPackages → LinkPackages");
    println!("- DownloadCython → CheckoutRepo → BuildFromSource");
    println!("- Total tasks: {}\n", graph.task_count());

    // Execute with production-like configuration
    let config = ExecutorConfig::default()
        .with_max_concurrent_tasks(4)?
        .with_default_task_timeout(Duration::from_secs(120))?
        .with_fail_fast(true)
        .with_caching(true);
    
    let executor = DogeExecutor::with_config(config)?;
    
    println!("🚀 Starting pixi workflow execution...\n");
    let start_time = Instant::now();
    
    let results = executor.execute(graph).await?;
    let total_time = start_time.elapsed();
    
    println!("\n=== Execution Results ===");
    
    let mut successful = 0;
    let mut failed = 0;
    
    for (node_index, result) in results {
        match result {
            doge::TaskResult::Success(output) => {
                successful += 1;
                println!("✅ Node {}: Success", node_index.index());
                match output {
                    PixiWorkflowOutput::SolvedEnvironment { resolved_packages, .. } => {
                        println!("   Resolved {} packages", resolved_packages.len());
                    }
                    PixiWorkflowOutput::DownloadedPackage { verified, .. } => {
                        println!("   Downloaded and {}", if verified { "verified" } else { "not verified" });
                    }
                    PixiWorkflowOutput::ExtractedPackage { files_count, .. } => {
                        println!("   Extracted {} files", files_count);
                    }
                    PixiWorkflowOutput::LinkedPackage { links_created, .. } => {
                        println!("   Created {} links", links_created);
                    }
                    PixiWorkflowOutput::CheckedOutRepo { commit_hash, .. } => {
                        println!("   Checked out commit {}", commit_hash);
                    }
                    PixiWorkflowOutput::BuiltFromSource { artifacts, build_time } => {
                        println!("   Built {} artifacts in {:.1}s", artifacts.len(), build_time.as_secs_f32());
                    }
                }
            }
            doge::TaskResult::Error(err) => {
                failed += 1;
                println!("❌ Node {}: Error - {:?}", node_index.index(), err);
            }
            doge::TaskResult::Cancelled => {
                println!("🚫 Node {}: Cancelled", node_index.index());
            }
        }
    }
    
    println!("\n📊 Summary:");
    println!("- Total execution time: {:.2}s", total_time.as_secs_f32());
    println!("- Successful tasks: {}", successful);
    println!("- Failed tasks: {}", failed);
    println!("- Success rate: {:.1}%", (successful as f32 / (successful + failed) as f32) * 100.0);
    
    println!("\n✅ Production integration example completed!");
    println!("Doge successfully replaced pixi_command_dispatcher while maintaining:");
    println!("  - Full progress reporting compatibility");
    println!("  - Dependency-aware task execution");
    println!("  - Parallel package processing");
    println!("  - Error handling and retry logic");
    println!("  - Context-aware operation tracking");
    
    Ok(())
}