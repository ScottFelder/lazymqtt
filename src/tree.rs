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
