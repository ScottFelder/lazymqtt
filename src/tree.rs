use crate::mqtt::Message;
use std::collections::BTreeMap;

/// A node in the topic hierarchy (split on '/').
#[derive(Default)]
pub struct TopicNode {
    pub children: BTreeMap<String, TopicNode>,
    pub last: Option<Message>,
    pub count: usize,
}

#[derive(Default)]
pub struct TopicTree {
    pub root: TopicNode,
}

impl TopicTree {
    pub fn insert(&mut self, msg: Message) {
        let parts: Vec<&str> = msg.topic.split('/').collect();
        let mut node = &mut self.root;
        for p in &parts {
            node = node.children.entry(p.to_string()).or_default();
        }
        node.count += 1;
        node.last = Some(msg);
    }

    pub fn clear(&mut self) {
        self.root = TopicNode::default();
    }

    /// Remove the node at `path` (slash-separated) and its whole subtree.
    /// Returns true if a node was actually removed.
    pub fn remove(&mut self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        let Some((last, parents)) = parts.split_last() else {
            return false;
        };
        let mut node = &mut self.root;
        for p in parents {
            match node.children.get_mut(*p) {
                Some(child) => node = child,
                None => return false,
            }
        }
        node.children.remove(*last).is_some()
    }

    /// Flatten the tree into displayable rows honoring per-node expansion state.
    pub fn rows(&self, expanded: &std::collections::HashSet<String>) -> Vec<TreeRow> {
        let mut out = Vec::new();
        walk(&self.root, String::new(), 0, expanded, &mut out);
        out
    }
}

pub struct TreeRow {
    pub path: String,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub count: usize,
    pub value: Option<String>,
}

fn walk(
    node: &TopicNode,
    path: String,
    depth: usize,
    expanded: &std::collections::HashSet<String>,
    out: &mut Vec<TreeRow>,
) {
    for (name, child) in &node.children {
        let child_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", path, name)
        };
        let has_children = !child.children.is_empty();
        let is_expanded = expanded.contains(&child_path);
        out.push(TreeRow {
            path: child_path.clone(),
            label: name.clone(),
            depth,
            has_children,
            expanded: is_expanded,
            count: child.count,
            value: child.last.as_ref().map(|m| m.payload.clone()),
        });
        if has_children && is_expanded {
            walk(child, child_path, depth + 1, expanded, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn msg(topic: &str) -> Message {
        Message {
            id: 0,
            topic: topic.into(),
            payload: "x".into(),
            qos: 0,
            retained: false,
            time: Local::now(),
        }
    }

    fn paths(tree: &TopicTree) -> Vec<String> {
        let all = std::collections::HashSet::new();
        tree.rows(&all).into_iter().map(|r| r.path).collect()
    }

    #[test]
    fn remove_prunes_node_and_subtree() {
        let mut tree = TopicTree::default();
        tree.insert(msg("sensor/temp"));
        tree.insert(msg("sensor/humidity"));
        tree.insert(msg("status"));

        // Removing a parent drops it and every descendant, leaving siblings.
        assert!(tree.remove("sensor"));
        assert_eq!(paths(&tree), vec!["status".to_string()]);
    }

    #[test]
    fn remove_leaf_leaves_siblings() {
        let mut tree = TopicTree::default();
        tree.insert(msg("a/b"));
        tree.insert(msg("a/c"));

        // Expand the parent so the child rows are visible, then drop one leaf.
        let mut expanded = std::collections::HashSet::new();
        expanded.insert("a".to_string());
        assert!(tree.remove("a/b"));
        let visible: Vec<String> = tree.rows(&expanded).into_iter().map(|r| r.path).collect();
        assert_eq!(visible, vec!["a".to_string(), "a/c".to_string()]);
    }

    #[test]
    fn remove_missing_path_is_noop() {
        let mut tree = TopicTree::default();
        tree.insert(msg("a/b"));
        assert!(!tree.remove("a/x"));
        assert!(!tree.remove("nope"));
        assert_eq!(paths(&tree), vec!["a".to_string()]);
    }
}
