//! Task graph management using daggy for DAG operations

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;

use daggy::{Dag, Walker};
use daggy::petgraph::visit::{IntoNodeIdentifiers, IntoEdgeReferences, EdgeRef};
use serde::{Deserialize, Serialize};

use crate::error::{DogeError, GraphError};
use crate::task::{AsyncTask, TaskId, TaskMetadata, TaskPriority};

/// Node index in the DAG, re-exported from daggy
pub use daggy::NodeIndex;

/// A node in the task graph containing a task and its metadata
#[derive(Debug, Clone)]
pub struct TaskNode<T> {
    /// The task to be executed
    pub task: T,
    /// Metadata about the task
    pub metadata: TaskMetadata,
    /// Whether this task has been deduplicated with another
    pub is_deduplicated: bool,
    /// Original node that this was deduplicated from (if any)
    pub deduplicated_from: Option<NodeIndex>,
}

impl<T: AsyncTask> TaskNode<T> {
    /// Create a new task node
    pub fn new(task: T) -> Self {
        let metadata = TaskMetadata::new(&task);
        Self {
            task,
            metadata,
            is_deduplicated: false,
            deduplicated_from: None,
        }
    }

    /// Create a deduplicated task node that points to an original
    pub fn deduplicated(task: T, original: NodeIndex) -> Self {
        let metadata = TaskMetadata::new(&task);
        Self {
            task,
            metadata,
            is_deduplicated: true,
            deduplicated_from: Some(original),
        }
    }
}

/// A directed acyclic graph of tasks with dependency management
#[derive(Debug)]
pub struct TaskGraph<T> {
    /// The underlying DAG structure
    dag: Dag<TaskNode<T>, ()>,
    /// Map from task content to node index for deduplication
    task_map: HashMap<T, NodeIndex>,
    /// Map from task ID to node index
    id_map: HashMap<TaskId, NodeIndex>,
    /// Nodes that are ready to execute (no pending dependencies)
    ready_nodes: VecDeque<NodeIndex>,
    /// Nodes that are currently executing
    executing_nodes: HashSet<NodeIndex>,
    /// Nodes that have completed successfully
    completed_nodes: HashSet<NodeIndex>,
    /// Nodes that have failed
    failed_nodes: HashSet<NodeIndex>,
}

impl<T> Default for TaskGraph<T> 
where 
    T: AsyncTask,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TaskGraph<T> 
where 
    T: AsyncTask,
{
    /// Create a new empty task graph
    pub fn new() -> Self {
        Self {
            dag: Dag::new(),
            task_map: HashMap::new(),
            id_map: HashMap::new(),
            ready_nodes: VecDeque::new(),
            executing_nodes: HashSet::new(),
            completed_nodes: HashSet::new(),
            failed_nodes: HashSet::new(),
        }
    }

    /// Add a task to the graph, returning its node index
    /// 
    /// If an identical task already exists, this will return the existing node
    /// and mark this as a deduplicated request.
    pub fn add_task(&mut self, task: T) -> NodeIndex {
        // Check if we already have this exact task
        if let Some(&existing_node) = self.task_map.get(&task) {
            tracing::debug!(
                task = %task.name(),
                existing_node = ?existing_node,
                "Task deduplicated"
            );
            return existing_node;
        }

        // Create new task node
        let node = TaskNode::new(task.clone());
        let task_id = node.metadata.id;
        
        // Add to DAG
        let node_index = self.dag.add_node(node);
        
        // Update mappings
        self.task_map.insert(task, node_index);
        self.id_map.insert(task_id, node_index);
        
        // Check if this task is ready to execute (no dependencies)
        if self.dag.parents(node_index).iter(&self.dag).count() == 0 {
            self.ready_nodes.push_back(node_index);
        }

        tracing::debug!(
            task_id = %task_id,
            node_index = ?node_index,
            "Added new task to graph"
        );

        node_index
    }

    /// Add a dependency relationship between two tasks
    /// 
    /// The `from` task must complete before the `to` task can start.
    pub fn add_dependency(&mut self, from: NodeIndex, to: NodeIndex) -> Result<(), DogeError> {
        // Check that both nodes exist
        if !self.dag.node_weight(from).is_some() {
            return Err(DogeError::Graph(GraphError::NodeNotFound(from)));
        }
        if !self.dag.node_weight(to).is_some() {
            return Err(DogeError::Graph(GraphError::NodeNotFound(to)));
        }

        // Check if adding this edge would create a cycle
        if self.would_create_cycle(from, to) {
            return Err(DogeError::Graph(GraphError::CycleDetected {
                from,
                to,
            }));
        }

        // Add the dependency edge
        self.dag.add_edge(from, to, ()).map_err(|_| {
            DogeError::Graph(GraphError::CycleDetected { from, to })
        })?;

        // Update metadata
        let from_id = self.dag.node_weight(from).unwrap().metadata.id;
        let to_id = self.dag.node_weight(to).unwrap().metadata.id;
        
        if let Some(from_node) = self.dag.node_weight_mut(from) {
            from_node.metadata.dependents.push(to_id);
        }
        if let Some(to_node) = self.dag.node_weight_mut(to) {
            to_node.metadata.dependencies.push(from_id);
        }

        // Remove 'to' node from ready queue if it was there
        self.ready_nodes.retain(|&node| node != to);

        tracing::debug!(
            from = ?from,
            to = ?to,
            "Added dependency relationship"
        );

        Ok(())
    }

    /// Check if adding an edge from `from` to `to` would create a cycle
    fn would_create_cycle(&self, from: NodeIndex, to: NodeIndex) -> bool {
        // If there's already a path from 'to' to 'from', adding 'from' -> 'to' would create a cycle
        self.has_path(to, from)
    }

    /// Check if there's a path from `start` to `end`
    fn has_path(&self, start: NodeIndex, end: NodeIndex) -> bool {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            if current == end {
                return true;
            }

            if visited.insert(current) {
                // Add all children to the queue
                for child in self.dag.children(current).iter(&self.dag) {
                    queue.push_back(child.1);
                }
            }
        }

        false
    }

    /// Get the next ready task to execute
    /// 
    /// Returns the highest priority task that has no pending dependencies.
    pub fn next_ready_task(&mut self) -> Option<NodeIndex> {
        if self.ready_nodes.is_empty() {
            return None;
        }

        // Find the highest priority task in the ready queue
        let mut best_idx = 0;
        let mut best_priority = TaskPriority::Low;

        for (idx, &node_index) in self.ready_nodes.iter().enumerate() {
            if let Some(node) = self.dag.node_weight(node_index) {
                if node.metadata.priority > best_priority {
                    best_priority = node.metadata.priority;
                    best_idx = idx;
                }
            }
        }

        // Remove and return the best task
        let node_index = self.ready_nodes.remove(best_idx)?;
        self.executing_nodes.insert(node_index);

        tracing::debug!(
            node = ?node_index,
            priority = ?best_priority,
            "Selected task for execution"
        );

        Some(node_index)
    }

    /// Mark a task as completed and update dependent tasks
    pub fn mark_completed(&mut self, node_index: NodeIndex) -> Result<(), DogeError> {
        if !self.executing_nodes.remove(&node_index) {
            return Err(DogeError::Graph(GraphError::TaskNotExecuting(node_index)));
        }

        self.completed_nodes.insert(node_index);

        // Update the task metadata
        if let Some(node) = self.dag.node_weight_mut(node_index) {
            node.metadata.mark_completed();
        }

        // Check if any dependent tasks are now ready
        self.update_ready_tasks(node_index);

        tracing::debug!(
            node = ?node_index,
            "Task marked as completed"
        );

        Ok(())
    }

    /// Mark a task as failed
    pub fn mark_failed(&mut self, node_index: NodeIndex, error: String) -> Result<(), DogeError> {
        if !self.executing_nodes.remove(&node_index) {
            return Err(DogeError::Graph(GraphError::TaskNotExecuting(node_index)));
        }

        self.failed_nodes.insert(node_index);

        // Update the task metadata
        if let Some(node) = self.dag.node_weight_mut(node_index) {
            node.metadata.mark_failed(error);
        }

        tracing::warn!(
            node = ?node_index,
            "Task marked as failed"
        );

        Ok(())
    }

    /// Update the ready tasks queue after a task completion
    fn update_ready_tasks(&mut self, completed_node: NodeIndex) {
        for child in self.dag.children(completed_node).iter(&self.dag) {
            let child_node = child.1;
            
            // Check if all dependencies of this child are completed
            let all_deps_completed = self.dag
                .parents(child_node)
                .iter(&self.dag)
                .all(|parent| self.completed_nodes.contains(&parent.1));

            // If all dependencies are completed and it's not already in our queues
            if all_deps_completed 
                && !self.ready_nodes.contains(&child_node)
                && !self.executing_nodes.contains(&child_node)
                && !self.completed_nodes.contains(&child_node)
                && !self.failed_nodes.contains(&child_node)
            {
                self.ready_nodes.push_back(child_node);
                tracing::debug!(
                    node = ?child_node,
                    "Task became ready for execution"
                );
            }
        }
    }

    /// Get a task node by its index
    pub fn get_node(&self, node_index: NodeIndex) -> Option<&TaskNode<T>> {
        self.dag.node_weight(node_index)
    }

    /// Get a mutable task node by its index
    pub fn get_node_mut(&mut self, node_index: NodeIndex) -> Option<&mut TaskNode<T>> {
        self.dag.node_weight_mut(node_index)
    }

    /// Get a task node by its ID
    pub fn get_node_by_id(&self, task_id: TaskId) -> Option<&TaskNode<T>> {
        self.id_map.get(&task_id)
            .and_then(|&node_index| self.dag.node_weight(node_index))
    }

    /// Get the total number of tasks in the graph
    pub fn task_count(&self) -> usize {
        self.dag.node_count()
    }

    /// Get the number of ready tasks
    pub fn ready_count(&self) -> usize {
        self.ready_nodes.len()
    }

    /// Get the number of executing tasks
    pub fn executing_count(&self) -> usize {
        self.executing_nodes.len()
    }

    /// Get the number of completed tasks
    pub fn completed_count(&self) -> usize {
        self.completed_nodes.len()
    }

    /// Get the number of failed tasks
    pub fn failed_count(&self) -> usize {
        self.failed_nodes.len()
    }

    /// Check if all tasks have finished (either completed or failed)
    pub fn is_finished(&self) -> bool {
        let total_finished = self.completed_nodes.len() + self.failed_nodes.len();
        total_finished == self.dag.node_count()
    }

    /// Check if all tasks completed successfully
    pub fn is_successful(&self) -> bool {
        self.is_finished() && self.failed_nodes.is_empty()
    }

    /// Get all node indices in topological order
    pub fn topological_order(&self) -> Vec<NodeIndex> {
        daggy::petgraph::algo::toposort(&self.dag, None)
            .unwrap_or_else(|_| Vec::new())
    }

    /// Get dependency nodes (parents) for a given node
    pub fn get_dependency_nodes(&self, node_index: NodeIndex) -> Vec<NodeIndex> {
        self.dag.parents(node_index).iter(&self.dag).map(|(_, parent_index)| parent_index).collect()
    }

    /// Get statistics about the graph
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            total_tasks: self.task_count(),
            ready_tasks: self.ready_count(),
            executing_tasks: self.executing_count(),
            completed_tasks: self.completed_count(),
            failed_tasks: self.failed_count(),
            is_finished: self.is_finished(),
            is_successful: self.is_successful(),
        }
    }

    /// Export the graph structure for visualization or debugging
    pub fn export_dot(&self) -> String 
    where 
        T: Debug,
    {
        use std::fmt::Write;
        
        let mut dot = String::new();
        writeln!(dot, "digraph TaskGraph {{").unwrap();
        writeln!(dot, "  rankdir=TB;").unwrap();
        
        // Add nodes
        for node_index in self.dag.node_identifiers() {
            if let Some(node) = self.dag.node_weight(node_index) {
                let color = if self.completed_nodes.contains(&node_index) {
                    "green"
                } else if self.failed_nodes.contains(&node_index) {
                    "red"
                } else if self.executing_nodes.contains(&node_index) {
                    "yellow"
                } else {
                    "lightblue"
                };
                
                writeln!(
                    dot,
                    "  {} [label=\"{}\\n{:?}\" fillcolor={} style=filled];",
                    node_index.index(),
                    node.metadata.name,
                    node.metadata.priority,
                    color
                ).unwrap();
            }
        }
        
        // Add edges
        for (from, to) in self.dag.edge_references().map(|edge| (edge.source(), edge.target())) {
            writeln!(dot, "  {} -> {};", from.index(), to.index()).unwrap();
        }
        
        writeln!(dot, "}}").unwrap();
        dot
    }
}

/// Statistics about a task graph
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_tasks: usize,
    pub ready_tasks: usize,
    pub executing_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub is_finished: bool,
    pub is_successful: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct TestTask {
        id: u32,
        name: String,
    }

    #[async_trait::async_trait]
    impl AsyncTask for TestTask {
        type Output = String;
        type Error = String;

        async fn execute(&self) -> Result<Self::Output, Self::Error> {
            Ok(format!("Executed {}", self.name))
        }

        fn name(&self) -> String {
            self.name.clone()
        }
    }

    #[test]
    fn test_empty_graph() {
        let graph: TaskGraph<TestTask> = TaskGraph::new();
        assert_eq!(graph.task_count(), 0);
        assert!(graph.is_finished());
        assert!(graph.is_successful());
    }

    #[test]
    fn test_add_task() {
        let mut graph = TaskGraph::new();
        let task = TestTask { id: 1, name: "test".to_string() };
        
        let node1 = graph.add_task(task.clone());
        assert_eq!(graph.task_count(), 1);
        assert_eq!(graph.ready_count(), 1);
        
        // Adding the same task should return the same node (deduplication)
        let node2 = graph.add_task(task);
        assert_eq!(node1, node2);
        assert_eq!(graph.task_count(), 1);
    }

    #[test]
    fn test_add_dependency() {
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, name: "task1".to_string() };
        let task2 = TestTask { id: 2, name: "task2".to_string() };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        // Both should be ready initially
        assert_eq!(graph.ready_count(), 2);
        
        // Add dependency: task1 -> task2
        graph.add_dependency(node1, node2).unwrap();
        
        // Now only task1 should be ready
        assert_eq!(graph.ready_count(), 1);
        
        // The ready task should be task1
        let ready_task = graph.next_ready_task().unwrap();
        assert_eq!(ready_task, node1);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, name: "task1".to_string() };
        let task2 = TestTask { id: 2, name: "task2".to_string() };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        // Add dependency: task1 -> task2
        graph.add_dependency(node1, node2).unwrap();
        
        // Try to add reverse dependency: task2 -> task1 (should fail)
        let result = graph.add_dependency(node2, node1);
        assert!(result.is_err());
        
        if let Err(DogeError::Graph(GraphError::CycleDetected { from, to })) = result {
            assert_eq!(from, node2);
            assert_eq!(to, node1);
        } else {
            panic!("Expected cycle detection error");
        }
    }

    #[test]
    fn test_task_completion_flow() {
        let mut graph = TaskGraph::new();
        
        let task1 = TestTask { id: 1, name: "task1".to_string() };
        let task2 = TestTask { id: 2, name: "task2".to_string() };
        
        let node1 = graph.add_task(task1);
        let node2 = graph.add_task(task2);
        
        graph.add_dependency(node1, node2).unwrap();
        
        // Initially only task1 is ready
        assert_eq!(graph.ready_count(), 1);
        assert_eq!(graph.executing_count(), 0);
        assert_eq!(graph.completed_count(), 0);
        
        // Get task1 for execution
        let ready_task = graph.next_ready_task().unwrap();
        assert_eq!(ready_task, node1);
        assert_eq!(graph.ready_count(), 0);
        assert_eq!(graph.executing_count(), 1);
        
        // Complete task1
        graph.mark_completed(node1).unwrap();
        assert_eq!(graph.executing_count(), 0);
        assert_eq!(graph.completed_count(), 1);
        assert_eq!(graph.ready_count(), 1); // task2 should now be ready
        
        // Get task2 for execution
        let ready_task = graph.next_ready_task().unwrap();
        assert_eq!(ready_task, node2);
        
        // Complete task2
        graph.mark_completed(node2).unwrap();
        assert_eq!(graph.completed_count(), 2);
        assert!(graph.is_finished());
        assert!(graph.is_successful());
    }

    #[test]
    fn test_task_failure() {
        let mut graph = TaskGraph::new();
        
        let task = TestTask { id: 1, name: "task".to_string() };
        let node = graph.add_task(task);
        
        let ready_task = graph.next_ready_task().unwrap();
        assert_eq!(ready_task, node);
        
        graph.mark_failed(node, "Test error".to_string()).unwrap();
        assert_eq!(graph.failed_count(), 1);
        assert!(graph.is_finished());
        assert!(!graph.is_successful());
    }

    #[test]
    fn test_priority_ordering() {
        let mut graph = TaskGraph::new();
        
        // Create tasks with different priorities
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        struct PriorityTask {
            id: u32,
            priority: TaskPriority,
        }

        #[async_trait::async_trait]
        impl AsyncTask for PriorityTask {
            type Output = u32;
            type Error = String;

            async fn execute(&self) -> Result<Self::Output, Self::Error> {
                Ok(self.id)
            }

            fn name(&self) -> String {
                format!("task_{}", self.id)
            }

            fn priority(&self) -> TaskPriority {
                self.priority
            }
        }
        
        let low_task = PriorityTask { id: 1, priority: TaskPriority::Low };
        let high_task = PriorityTask { id: 2, priority: TaskPriority::High };
        let normal_task = PriorityTask { id: 3, priority: TaskPriority::Normal };
        
        graph.add_task(low_task);
        graph.add_task(high_task);
        graph.add_task(normal_task);
        
        // Should get high priority task first
        let first = graph.next_ready_task().unwrap();
        assert_eq!(graph.get_node(first).unwrap().task.id, 2);
        
        // Then normal priority
        let second = graph.next_ready_task().unwrap();
        assert_eq!(graph.get_node(second).unwrap().task.id, 3);
        
        // Finally low priority
        let third = graph.next_ready_task().unwrap();
        assert_eq!(graph.get_node(third).unwrap().task.id, 1);
    }
}