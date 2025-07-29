//! Graph visualization example for the Doge task executor
//!
//! This example demonstrates how to visualize task graphs, inspect dependencies,
//! and export graphs in DOT format for external visualization tools.

use doge::{async_trait, AsyncTask, DogeExecutor, TaskGraph, TaskPriority};
use std::time::Duration;

/// A build task that represents different stages of a software build pipeline
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum BuildTask {
    /// Checkout source code
    Checkout { repo: String },
    /// Install dependencies
    InstallDeps { project: String },
    /// Run linting
    Lint { target: String },
    /// Run tests
    Test { suite: String },
    /// Build the project
    Build { config: String },
    /// Package the build
    Package { format: String },
    /// Deploy to environment
    Deploy { environment: String },
}

#[async_trait]
impl AsyncTask for BuildTask {
    type Output = String;
    type Error = String;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let (action, duration) = match self {
            BuildTask::Checkout { repo } => {
                (format!("Checked out {}", repo), 100)
            }
            BuildTask::InstallDeps { project } => {
                (format!("Installed dependencies for {}", project), 300)
            }
            BuildTask::Lint { target } => {
                (format!("Linted {}", target), 150)
            }
            BuildTask::Test { suite } => {
                (format!("Ran test suite: {}", suite), 500)
            }
            BuildTask::Build { config } => {
                (format!("Built with config: {}", config), 800)
            }
            BuildTask::Package { format } => {
                (format!("Packaged as {}", format), 200)
            }
            BuildTask::Deploy { environment } => {
                (format!("Deployed to {}", environment), 300)
            }
        };

        // Simulate work
        tokio::time::sleep(Duration::from_millis(duration)).await;
        
        println!("  ✅ {}", action);
        Ok(action)
    }

    fn name(&self) -> String {
        match self {
            BuildTask::Checkout { repo } => format!("checkout({})", repo),
            BuildTask::InstallDeps { project } => format!("install_deps({})", project),
            BuildTask::Lint { target } => format!("lint({})", target),
            BuildTask::Test { suite } => format!("test({})", suite),
            BuildTask::Build { config } => format!("build({})", config),
            BuildTask::Package { format } => format!("package({})", format),
            BuildTask::Deploy { environment } => format!("deploy({})", environment),
        }
    }

    fn priority(&self) -> TaskPriority {
        match self {
            BuildTask::Checkout { .. } => TaskPriority::Critical,
            BuildTask::InstallDeps { .. } => TaskPriority::High,
            BuildTask::Lint { .. } | BuildTask::Test { .. } => TaskPriority::Normal,
            BuildTask::Build { .. } => TaskPriority::High,
            BuildTask::Package { .. } => TaskPriority::Normal,
            BuildTask::Deploy { .. } => TaskPriority::Low,
        }
    }

    fn estimated_duration(&self) -> Option<Duration> {
        let ms = match self {
            BuildTask::Checkout { .. } => 100,
            BuildTask::InstallDeps { .. } => 300,
            BuildTask::Lint { .. } => 150,
            BuildTask::Test { .. } => 500,
            BuildTask::Build { .. } => 800,
            BuildTask::Package { .. } => 200,
            BuildTask::Deploy { .. } => 300,
        };
        Some(Duration::from_millis(ms))
    }
}

fn print_graph_analysis(graph: &TaskGraph<BuildTask>) {
    println!("📊 Graph Analysis:");
    println!("  Total tasks: {}", graph.task_count());
    println!("  Ready tasks: {}", graph.ready_count());
    println!("  Completed tasks: {}", graph.completed_count());
    println!("  Failed tasks: {}", graph.failed_count());
    println!("  Is finished: {}", graph.is_finished());
    println!("  Is successful: {}", graph.is_successful());
    
    // Print topological order
    println!("\n🔢 Topological Order:");
    let topo_order = graph.topological_order();
    for (i, node_index) in topo_order.iter().enumerate() {
        if let Some(node) = graph.get_node(*node_index) {
            println!("  {}. {} (Priority: {:?})", 
                     i + 1, 
                     node.task.name(), 
                     node.task.priority());
        }
    }
    
    // Print detailed task information
    println!("\n📋 Task Details:");
    for node_index in topo_order {
        if let Some(node) = graph.get_node(node_index) {
            println!("  📌 Task: {}", node.task.name());
            println!("     Node ID: {:?}", node_index);
            println!("     Priority: {:?}", node.task.priority());
            if let Some(duration) = node.task.estimated_duration() {
                println!("     Est. Duration: {:?}", duration);
            }
            println!("     Dependencies: {} tasks", node.metadata.dependencies.len());
            println!("     Dependents: {} tasks", node.metadata.dependents.len());
            println!();
        }
    }
}

fn print_dependency_tree(graph: &TaskGraph<BuildTask>) {
    println!("🌳 Dependency Tree:");
    
    let topo_order = graph.topological_order();
    for node_index in topo_order {
        if let Some(node) = graph.get_node(node_index) {
            // Only show nodes that don't have dependencies (root nodes)
            if node.metadata.dependencies.is_empty() {
                print_dependency_subtree(graph, node_index, 0);
            }
        }
    }
}

fn print_dependency_subtree(graph: &TaskGraph<BuildTask>, node_index: doge::NodeIndex, depth: usize) {
    let indent = "  ".repeat(depth);
    
    if let Some(node) = graph.get_node(node_index) {
        let priority_icon = match node.task.priority() {
            TaskPriority::Critical => "🔴",
            TaskPriority::High => "🟡",
            TaskPriority::Normal => "🟢",
            TaskPriority::Low => "🔵",
        };
        
        println!("{}├─ {} {} (Node {:?})", 
                 indent, 
                 priority_icon, 
                 node.task.name(), 
                 node_index);
        
        // Find and print dependents
        let topo_order = graph.topological_order();
        for dependent_node_index in topo_order {
            if let Some(dependent_node) = graph.get_node(dependent_node_index) {
                if dependent_node.metadata.dependencies.contains(&node.metadata.id) {
                    print_dependency_subtree(graph, dependent_node_index, depth + 1);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Graph Visualization Example ===\n");

    // Create a complex build pipeline graph
    let mut graph = TaskGraph::new();

    println!("🏗️  Building CI/CD Pipeline Graph...\n");

    // Phase 1: Source preparation
    let checkout = BuildTask::Checkout { repo: "my-project".to_string() };
    let install_deps = BuildTask::InstallDeps { project: "my-project".to_string() };
    
    let checkout_node = graph.add_task(checkout);
    let install_deps_node = graph.add_task(install_deps);
    
    // Checkout must happen before installing dependencies
    graph.add_dependency(checkout_node, install_deps_node)?;

    // Phase 2: Quality checks (can run in parallel after deps are installed)
    let lint = BuildTask::Lint { target: "src/".to_string() };
    let test_unit = BuildTask::Test { suite: "unit".to_string() };
    let test_integration = BuildTask::Test { suite: "integration".to_string() };
    
    let lint_node = graph.add_task(lint);
    let test_unit_node = graph.add_task(test_unit);
    let test_integration_node = graph.add_task(test_integration);
    
    // All quality checks depend on dependencies being installed
    graph.add_dependency(install_deps_node, lint_node)?;
    graph.add_dependency(install_deps_node, test_unit_node)?;
    graph.add_dependency(install_deps_node, test_integration_node)?;

    // Phase 3: Build (depends on all quality checks passing)
    let build_release = BuildTask::Build { config: "release".to_string() };
    let build_node = graph.add_task(build_release);
    
    graph.add_dependency(lint_node, build_node)?;
    graph.add_dependency(test_unit_node, build_node)?;
    graph.add_dependency(test_integration_node, build_node)?;

    // Phase 4: Packaging (depends on build)
    let package_docker = BuildTask::Package { format: "docker".to_string() };
    let package_rpm = BuildTask::Package { format: "rpm".to_string() };
    
    let package_docker_node = graph.add_task(package_docker);
    let package_rpm_node = graph.add_task(package_rpm);
    
    graph.add_dependency(build_node, package_docker_node)?;
    graph.add_dependency(build_node, package_rpm_node)?;

    // Phase 5: Deployment (depends on packaging)
    let deploy_staging = BuildTask::Deploy { environment: "staging".to_string() };
    let deploy_prod = BuildTask::Deploy { environment: "production".to_string() };
    
    let deploy_staging_node = graph.add_task(deploy_staging);
    let deploy_prod_node = graph.add_task(deploy_prod);
    
    // Staging deployment depends on docker package
    graph.add_dependency(package_docker_node, deploy_staging_node)?;
    
    // Production deployment depends on both packages and staging deployment
    graph.add_dependency(package_docker_node, deploy_prod_node)?;
    graph.add_dependency(package_rpm_node, deploy_prod_node)?;
    graph.add_dependency(deploy_staging_node, deploy_prod_node)?;

    println!("📈 Graph Construction Complete!\n");

    // Print comprehensive graph analysis
    print_graph_analysis(&graph);
    
    // Print dependency tree
    print_dependency_tree(&graph);
    
    // Export DOT format for external visualization
    println!("🎨 DOT Graph Export:");
    println!("  (Can be visualized with Graphviz: dot -Tpng graph.dot -o graph.png)");
    println!();
    
    let dot_output = graph.export_dot();
    println!("{}", dot_output);
    
    // Demonstrate graph statistics tracking during execution
    println!("\n🚀 Executing Pipeline with Live Statistics...\n");
    
    let executor = DogeExecutor::new();
    let (mut handle, results) = executor.execute_with_progress(graph).await?;
    
    // Monitor execution with periodic statistics
    let mut task_count = 0;
    let start_time = std::time::Instant::now();
    
    while let Some(event) = handle.next_progress_event().await {
        match event {
            doge::executor::ProgressEvent::Started { total_tasks } => {
                println!("📊 Pipeline started with {} total tasks", total_tasks);
                task_count = total_tasks;
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
                let elapsed = start_time.elapsed();
                println!("📈 Progress: {:.1}% ({}/{} completed, {} executing, {:.2} tasks/sec)", 
                         progress.progress_percentage * 100.0,
                         progress.completed_tasks,
                         progress.total_tasks,
                         progress.executing_tasks,
                         progress.completed_tasks as f64 / elapsed.as_secs_f64());
            }
            doge::executor::ProgressEvent::Completed { stats } => {
                println!("\n🎉 Pipeline completed successfully!");
                println!("   📊 Final Statistics:");
                println!("     Total time: {:?}", stats.total_duration);
                println!("     Tasks/second: {:.2}", stats.tasks_per_second);
                println!("     Peak concurrent: {}", stats.peak_concurrent_tasks);
                println!("     Successful tasks: {}", stats.successful_tasks);
                println!("     Failed tasks: {}", stats.failed_tasks);
                break;
            }
            doge::executor::ProgressEvent::Failed { error, stats } => {
                println!("\n💥 Pipeline failed: {}", error);
                println!("   📊 Final Statistics:");
                println!("     Total time: {:?}", stats.total_duration);
                println!("     Successful tasks: {}", stats.successful_tasks);
                println!("     Failed tasks: {}", stats.failed_tasks);
                break;
            }
            _ => {}
        }
    }
    
    // Summary of results
    println!("\n=== Execution Summary ===");
    let success_count = results.values().filter(|r| r.is_success()).count();
    let error_count = results.values().filter(|r| r.is_error()).count();
    let cancelled_count = results.values().filter(|r| r.is_cancelled()).count();
    
    println!("Results: {} successful, {} failed, {} cancelled", 
             success_count, error_count, cancelled_count);
    
    if success_count == task_count {
        println!("🎉 All pipeline stages completed successfully!");
        println!("   Ready for production deployment! 🚀");
    } else {
        println!("⚠️   Pipeline had issues - check failed tasks above.");
    }

    println!("\n=== Graph Visualization Example Complete ===");
    Ok(())
}