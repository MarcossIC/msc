//! Process tree construction and manipulation.
//!
//! Builds hierarchical process trees from flat process lists.

use super::metrics::ProcessMetrics;
use std::collections::HashMap;

/// A node in the process tree
#[derive(Debug, Clone)]
pub struct ProcessTreeNode {
    pub process: ProcessMetrics,
    pub children: Vec<ProcessTreeNode>,
    pub depth: usize,
}

/// Build a process tree from a flat list of processes
pub fn build_process_tree(processes: &[ProcessMetrics]) -> Vec<ProcessTreeNode> {
    // Create a map of PID -> Process for quick lookup
    let process_map: HashMap<u32, &ProcessMetrics> =
        processes.iter().map(|p| (p.pid, p)).collect();

    // Group processes by parent PID
    let mut children_map: HashMap<Option<u32>, Vec<&ProcessMetrics>> = HashMap::new();
    for process in processes {
        children_map
            .entry(process.parent_pid)
            .or_default()
            .push(process);
    }

    // Build tree recursively, starting with processes that have no parent
    // or whose parent is not in the list
    let mut roots = Vec::new();

    for process in processes {
        // Check if this is a root process
        let is_root = match process.parent_pid {
            None => true,
            Some(ppid) => !process_map.contains_key(&ppid),
        };

        if is_root {
            roots.push(build_node(process, &children_map, 0));
        }
    }

    // Sort roots by CPU usage (descending)
    roots.sort_by(|a, b| {
        b.process
            .cpu_usage_percent
            .partial_cmp(&a.process.cpu_usage_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    roots
}

/// Recursively build a tree node
fn build_node(
    process: &ProcessMetrics,
    children_map: &HashMap<Option<u32>, Vec<&ProcessMetrics>>,
    depth: usize,
) -> ProcessTreeNode {
    let mut children = Vec::new();

    if let Some(child_list) = children_map.get(&Some(process.pid)) {
        for &child in child_list {
            children.push(build_node(child, children_map, depth + 1));
        }
    }

    // Sort children by CPU usage (descending)
    children.sort_by(|a, b| {
        b.process
            .cpu_usage_percent
            .partial_cmp(&a.process.cpu_usage_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    ProcessTreeNode {
        process: process.clone(),
        children,
        depth,
    }
}

/// Flatten a process tree into a linear list with depth information
pub fn flatten_tree(tree: &[ProcessTreeNode]) -> Vec<FlattenedProcess> {
    let mut result = Vec::new();
    for node in tree {
        flatten_node(node, &mut result, true, Vec::new());
    }
    result
}

/// A flattened process with indentation information
#[derive(Debug, Clone)]
pub struct FlattenedProcess {
    pub process: ProcessMetrics,
    pub depth: usize,
    pub is_last: bool,
    pub parent_chain: Vec<bool>, // For drawing tree lines
}

fn flatten_node(
    node: &ProcessTreeNode,
    result: &mut Vec<FlattenedProcess>,
    is_last: bool,
    mut parent_chain: Vec<bool>,
) {
    result.push(FlattenedProcess {
        process: node.process.clone(),
        depth: node.depth,
        is_last,
        parent_chain: parent_chain.clone(),
    });

    if !node.children.is_empty() {
        parent_chain.push(is_last);
        let num_children = node.children.len();

        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == num_children - 1;
            flatten_node(child, result, child_is_last, parent_chain.clone());
        }
    }
}

/// Generate tree indentation string (like htop)
pub fn format_tree_indent(flattened: &FlattenedProcess) -> String {
    let mut indent = String::new();

    for &is_parent_last in &flattened.parent_chain {
        if is_parent_last {
            indent.push_str("  ");
        } else {
            indent.push_str("│ ");
        }
    }

    if flattened.depth > 0 {
        if flattened.is_last {
            indent.push_str("└─");
        } else {
            indent.push_str("├─");
        }
    }

    indent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_tree() {
        let processes = vec![
            ProcessMetrics {
                pid: 1,
                parent_pid: None,
                name: "init".to_string(),
                cpu_usage_percent: 0.1,
                memory_bytes: 1000,
                memory_percent: 0.01,
                status: "Running".to_string(),
                disk_read_bytes: 0,
                disk_write_bytes: 0,
            },
            ProcessMetrics {
                pid: 2,
                parent_pid: Some(1),
                name: "child1".to_string(),
                cpu_usage_percent: 5.0,
                memory_bytes: 2000,
                memory_percent: 0.02,
                status: "Running".to_string(),
                disk_read_bytes: 0,
                disk_write_bytes: 0,
            },
            ProcessMetrics {
                pid: 3,
                parent_pid: Some(1),
                name: "child2".to_string(),
                cpu_usage_percent: 3.0,
                memory_bytes: 1500,
                memory_percent: 0.015,
                status: "Running".to_string(),
                disk_read_bytes: 0,
                disk_write_bytes: 0,
            },
        ];

        let tree = build_process_tree(&processes);
        assert_eq!(tree.len(), 1); // One root
        assert_eq!(tree[0].process.pid, 1);
        assert_eq!(tree[0].children.len(), 2);
    }

    #[test]
    fn test_flatten_tree() {
        let processes = vec![
            ProcessMetrics {
                pid: 1,
                parent_pid: None,
                name: "init".to_string(),
                cpu_usage_percent: 0.1,
                memory_bytes: 1000,
                memory_percent: 0.01,
                status: "Running".to_string(),
                disk_read_bytes: 0,
                disk_write_bytes: 0,
            },
            ProcessMetrics {
                pid: 2,
                parent_pid: Some(1),
                name: "child1".to_string(),
                cpu_usage_percent: 5.0,
                memory_bytes: 2000,
                memory_percent: 0.02,
                status: "Running".to_string(),
                disk_read_bytes: 0,
                disk_write_bytes: 0,
            },
        ];

        let tree = build_process_tree(&processes);
        let flat = flatten_tree(&tree);

        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].depth, 0);
        assert_eq!(flat[1].depth, 1);
    }
}
