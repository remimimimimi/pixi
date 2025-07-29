//! Task execution context for accessing dependency results

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use crate::graph::NodeIndex;
use crate::task::TaskId;

/// Context provided to tasks during execution that allows access to dependency results
#[derive(Clone)]
pub struct TaskContext {
    /// Results from completed dependency tasks, keyed by node index
    dependency_results: Arc<HashMap<NodeIndex, Box<dyn Any + Send + Sync>>>,
    /// Map from task ID to node index for easier lookups
    task_id_to_node: Arc<HashMap<TaskId, NodeIndex>>,
}

impl TaskContext {
    /// Create a new empty task context
    pub fn new() -> Self {
        Self {
            dependency_results: Arc::new(HashMap::new()),
            task_id_to_node: Arc::new(HashMap::new()),
        }
    }

    /// Create a task context with dependency results
    pub fn with_dependencies(
        dependency_results: HashMap<NodeIndex, Box<dyn Any + Send + Sync>>,
        task_id_to_node: HashMap<TaskId, NodeIndex>,
    ) -> Self {
        Self {
            dependency_results: Arc::new(dependency_results),
            task_id_to_node: Arc::new(task_id_to_node),
        }
    }

    /// Get the result from a dependency task by node index
    pub fn get_dependency_result<T: Clone + Send + Sync + 'static>(
        &self,
        node_index: NodeIndex,
    ) -> Option<T> {
        self.dependency_results
            .get(&node_index)?
            .downcast_ref::<T>()
            .cloned()
    }

    /// Get the result from a dependency task by task ID
    pub fn get_dependency_result_by_id<T: Clone + Send + Sync + 'static>(
        &self,
        task_id: TaskId,
    ) -> Option<T> {
        let node_index = self.task_id_to_node.get(&task_id)?;
        self.get_dependency_result(*node_index)
    }

    /// Get all dependency results as a map of node indices to type-erased results
    pub fn get_all_dependency_results(&self) -> &HashMap<NodeIndex, Box<dyn Any + Send + Sync>> {
        &self.dependency_results
    }

    /// Check if a dependency result exists for the given node index
    pub fn has_dependency_result(&self, node_index: NodeIndex) -> bool {
        self.dependency_results.contains_key(&node_index)
    }

    /// Get the number of available dependency results
    pub fn dependency_count(&self) -> usize {
        self.dependency_results.len()
    }

    /// List all available dependency node indices
    pub fn dependency_nodes(&self) -> Vec<NodeIndex> {
        self.dependency_results.keys().copied().collect()
    }
}

impl Default for TaskContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskId;

    #[test]
    fn test_empty_context() {
        let context = TaskContext::new();
        assert_eq!(context.dependency_count(), 0);
        assert!(context.dependency_nodes().is_empty());
        assert!(!context.has_dependency_result(NodeIndex::new(0)));
    }

    #[test]
    fn test_context_with_dependencies() {
        let mut results = HashMap::new();
        let mut id_map = HashMap::new();
        
        let node1 = NodeIndex::new(0);
        let node2 = NodeIndex::new(1);
        let task_id1 = TaskId::new();
        let task_id2 = TaskId::new();
        
        results.insert(node1, Box::new(42i32) as Box<dyn Any + Send + Sync>);
        results.insert(node2, Box::new("hello".to_string()) as Box<dyn Any + Send + Sync>);
        
        id_map.insert(task_id1, node1);
        id_map.insert(task_id2, node2);
        
        let context = TaskContext::with_dependencies(results, id_map);
        
        assert_eq!(context.dependency_count(), 2);
        assert!(context.has_dependency_result(node1));
        assert!(context.has_dependency_result(node2));
        
        let result1: Option<i32> = context.get_dependency_result(node1);
        assert_eq!(result1, Some(42));
        
        let result2: Option<String> = context.get_dependency_result(node2);
        assert_eq!(result2, Some("hello".to_string()));
        
        let result_by_id: Option<i32> = context.get_dependency_result_by_id(task_id1);
        assert_eq!(result_by_id, Some(42));
        
        // Test type safety - asking for wrong type returns None
        let wrong_type: Option<String> = context.get_dependency_result(node1);
        assert_eq!(wrong_type, None);
    }

    #[test]
    fn test_dependency_listing() {
        let mut results = HashMap::new();
        let node1 = NodeIndex::new(5);
        let node2 = NodeIndex::new(10);
        
        results.insert(node1, Box::new(42i32) as Box<dyn Any + Send + Sync>);
        results.insert(node2, Box::new("test".to_string()) as Box<dyn Any + Send + Sync>);
        
        let context = TaskContext::with_dependencies(results, HashMap::new());
        
        let mut nodes = context.dependency_nodes();
        nodes.sort_by_key(|n| n.index());
        
        assert_eq!(nodes, vec![node1, node2]);
    }
}