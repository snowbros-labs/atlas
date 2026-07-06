//! The [`SemanticGraph`] wrapper around petgraph.
//!
//! Uses `StableDiGraph` so node handles stay valid across removals during
//! incremental updates. All query results are deterministically ordered.

use std::collections::HashMap;

use petgraph::algo::{tarjan_scc, toposort};
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::{Bfs, Reversed};
use petgraph::Direction;

use crate::model::{EdgeKind, Node, NodeId, NodeKind};

/// The project-wide semantic graph.
#[derive(Debug, Default)]
pub struct SemanticGraph {
    inner: StableDiGraph<Node, EdgeKind>,
    /// Label → node lookup for deduplicated insertion.
    index: HashMap<String, NodeId>,
}

impl SemanticGraph {
    /// Creates an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Adds a node, deduplicating by label: adding the same file or
    /// symbol twice returns the existing handle.
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let key = node.label();
        if let Some(&id) = self.index.get(&key) {
            return id;
        }
        let idx = self.inner.add_node(node);
        let id = NodeId(idx.index() as u32);
        self.index.insert(key, id);
        id
    }

    /// Adds a typed edge. Parallel edges of the same kind are
    /// deduplicated.
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) {
        let (a, b) = (Self::ix(from), Self::ix(to));
        let exists = self
            .inner
            .edges_connecting(a, b)
            .any(|e| *e.weight() == kind);
        if !exists {
            self.inner.add_edge(a, b, kind);
        }
    }

    /// Returns the node behind a handle, if it still exists.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.inner.node_weight(Self::ix(id))
    }

    /// Finds a node by its label.
    pub fn find(&self, label: &str) -> Option<NodeId> {
        self.index.get(label).copied()
    }

    /// All strongly-connected components with more than one node —
    /// i.e. circular dependency groups. Deterministic: components and
    /// their members are sorted by node id.
    pub fn circular_groups(&self) -> Vec<Vec<NodeId>> {
        let mut groups: Vec<Vec<NodeId>> = tarjan_scc(&self.inner)
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let mut ids: Vec<NodeId> = scc.into_iter().map(Self::id).collect();
                ids.sort();
                ids
            })
            .collect();
        groups.sort();
        groups
    }

    /// Topological order of the graph, or `None` when cycles exist.
    pub fn topo_order(&self) -> Option<Vec<NodeId>> {
        toposort(&self.inner, None)
            .ok()
            .map(|order| order.into_iter().map(Self::id).collect())
    }

    /// Everything that (transitively) depends on `target`: reverse
    /// reachability. Answers "what breaks if I change this?". The target
    /// itself is not included. Result is sorted.
    pub fn dependents_of(&self, target: NodeId) -> Vec<NodeId> {
        let reversed = Reversed(&self.inner);
        let mut bfs = Bfs::new(reversed, Self::ix(target));
        let mut out = Vec::new();
        while let Some(ix) = bfs.next(reversed) {
            if ix != Self::ix(target) {
                out.push(Self::id(ix));
            }
        }
        out.sort();
        out
    }

    /// Nodes with no incoming edges of the given kind — e.g. files
    /// nothing imports (dead-file candidates). Sorted.
    pub fn roots(&self, kind: EdgeKind) -> Vec<NodeId> {
        let mut out: Vec<NodeId> = self
            .inner
            .node_indices()
            .filter(|&ix| {
                !self
                    .inner
                    .edges_directed(ix, Direction::Incoming)
                    .any(|e| *e.weight() == kind)
            })
            .map(Self::id)
            .collect();
        out.sort();
        out
    }

    /// Exports the graph in Graphviz DOT format (deterministic ordering).
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph snowbros {\n");
        let mut indices: Vec<NodeIndex> = self.inner.node_indices().collect();
        indices.sort();
        for ix in &indices {
            let node = &self.inner[*ix];
            dot.push_str(&format!(
                "    n{} [label=\"{}\"];\n",
                ix.index(),
                node.label().replace('"', "\\\"")
            ));
        }
        let mut edges: Vec<(usize, usize, EdgeKind)> = self
            .inner
            .edge_indices()
            .filter_map(|e| {
                let (a, b) = self.inner.edge_endpoints(e)?;
                Some((a.index(), b.index(), self.inner[e]))
            })
            .collect();
        edges.sort();
        for (a, b, kind) in edges {
            dot.push_str(&format!("    n{a} -> n{b} [label=\"{kind}\"];\n"));
        }
        dot.push_str("}\n");
        dot
    }

    /// Iterates all file nodes (sorted by path) — common analyzer input.
    pub fn files(&self) -> Vec<(NodeId, &Node)> {
        self.nodes_of(|kind| matches!(kind, NodeKind::File { .. }))
    }

    /// Iterates all external package nodes, sorted by node id.
    pub fn packages(&self) -> Vec<(NodeId, &Node)> {
        self.nodes_of(|kind| matches!(kind, NodeKind::Package { .. }))
    }

    /// Iterates all symbol nodes (declared functions/classes/bindings),
    /// sorted by node id — the input to symbol-level tooling.
    pub fn symbols(&self) -> Vec<(NodeId, &Node)> {
        self.nodes_of(|kind| matches!(kind, NodeKind::Symbol { .. }))
    }

    /// Whether a node has at least one incoming edge of the given kind.
    pub fn has_incoming(&self, id: NodeId, kind: EdgeKind) -> bool {
        self.inner
            .edges_directed(Self::ix(id), Direction::Incoming)
            .any(|e| *e.weight() == kind)
    }

    /// Whether a node has at least one outgoing edge of the given kind.
    pub fn has_outgoing(&self, id: NodeId, kind: EdgeKind) -> bool {
        self.inner
            .edges_directed(Self::ix(id), Direction::Outgoing)
            .any(|e| *e.weight() == kind)
    }

    fn nodes_of(&self, pred: impl Fn(&NodeKind) -> bool) -> Vec<(NodeId, &Node)> {
        let mut out: Vec<(NodeId, &Node)> = self
            .inner
            .node_indices()
            .filter(|&ix| pred(&self.inner[ix].kind))
            .map(|ix| (Self::id(ix), &self.inner[ix]))
            .collect();
        out.sort_by_key(|(id, _)| *id);
        out
    }

    fn ix(id: NodeId) -> NodeIndex {
        NodeIndex::new(id.0 as usize)
    }

    fn id(ix: NodeIndex) -> NodeId {
        NodeId(ix.index() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a → b → c, plus d ↔ e cycle, f isolated.
    fn sample() -> (SemanticGraph, Vec<NodeId>) {
        let mut g = SemanticGraph::new();
        let ids: Vec<NodeId> = ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts", "f.ts"]
            .iter()
            .map(|p| g.add_node(Node::file(*p)))
            .collect();
        g.add_edge(ids[0], ids[1], EdgeKind::Imports);
        g.add_edge(ids[1], ids[2], EdgeKind::Imports);
        g.add_edge(ids[3], ids[4], EdgeKind::Imports);
        g.add_edge(ids[4], ids[3], EdgeKind::Imports);
        (g, ids)
    }

    #[test]
    fn dedup_nodes_by_label() {
        let mut g = SemanticGraph::new();
        let a1 = g.add_node(Node::file("src/a.ts"));
        let a2 = g.add_node(Node::file("src/a.ts"));
        assert_eq!(a1, a2);
        assert_eq!(g.node_count(), 1);
    }

    #[test]
    fn dedup_edges_by_kind() {
        let mut g = SemanticGraph::new();
        let a = g.add_node(Node::file("a.ts"));
        let b = g.add_node(Node::file("b.ts"));
        g.add_edge(a, b, EdgeKind::Imports);
        g.add_edge(a, b, EdgeKind::Imports);
        g.add_edge(a, b, EdgeKind::TypeRef);
        assert_eq!(g.edge_count(), 2);
    }

    #[test]
    fn detects_circular_group() {
        let (g, ids) = sample();
        let groups = g.circular_groups();
        assert_eq!(groups, vec![vec![ids[3], ids[4]]]);
    }

    #[test]
    fn toposort_none_with_cycle_some_without() {
        let (g, _) = sample();
        assert!(g.topo_order().is_none());

        let mut acyclic = SemanticGraph::new();
        let a = acyclic.add_node(Node::file("a.ts"));
        let b = acyclic.add_node(Node::file("b.ts"));
        acyclic.add_edge(a, b, EdgeKind::Imports);
        let order = acyclic.topo_order().unwrap();
        let pos = |id| order.iter().position(|&x| x == id).unwrap();
        assert!(pos(a) < pos(b));
    }

    #[test]
    fn impact_analysis_via_reverse_reachability() {
        let (g, ids) = sample();
        // c is imported by b, which is imported by a → changing c impacts a and b.
        assert_eq!(g.dependents_of(ids[2]), vec![ids[0], ids[1]]);
        // Nothing depends on a.
        assert!(g.dependents_of(ids[0]).is_empty());
    }

    #[test]
    fn roots_finds_unimported_files() {
        let (g, ids) = sample();
        // a starts a chain; d/e form a cycle (both imported); f isolated.
        assert_eq!(g.roots(EdgeKind::Imports), vec![ids[0], ids[5]]);
    }

    #[test]
    fn dot_export_is_deterministic() {
        let (g, _) = sample();
        let dot1 = g.to_dot();
        let dot2 = g.to_dot();
        assert_eq!(dot1, dot2);
        assert!(dot1.contains("digraph snowbros"));
        assert!(dot1.contains("label=\"imports\""));
    }

    #[test]
    fn find_by_label() {
        let (g, ids) = sample();
        assert_eq!(g.find("b.ts"), Some(ids[1]));
        assert_eq!(g.find("nope.ts"), None);
    }
}
