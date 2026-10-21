use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::Direction;
use std::collections::{HashMap, HashSet, VecDeque};

struct DisjointSet {
    parent: HashMap<NodeId, NodeId>,
    rank: HashMap<NodeId, usize>,
}

impl DisjointSet {
    fn new() -> Self {
        Self {
            parent: HashMap::new(),
            rank: HashMap::new(),
        }
    }

    fn make_set(&mut self, node: NodeId) {
        self.parent.insert(node, node);
        self.rank.insert(node, 0);
    }

    fn find(&mut self, node: NodeId) -> NodeId {
        let p = self.parent[&node];
        if p != node {
            let root = self.find(p);
            self.parent.insert(node, root);
            root
        } else {
            p
        }
    }

    fn union(&mut self, u: NodeId, v: NodeId) {
        let root_u = self.find(u);
        let root_v = self.find(v);
        if root_u == root_v {
            return;
        }

        let rank_u = self.rank[&root_u];
        let rank_v = self.rank[&root_v];

        if rank_u < rank_v {
            self.parent.insert(root_u, root_v);
        } else if rank_u > rank_v {
            self.parent.insert(root_v, root_u);
        } else {
            self.parent.insert(root_v, root_u);
            self.rank.insert(root_u, rank_u + 1);
        }
    }
}

/// Detect Weakly Connected Components (WCC) in the graph using Disjoint-Set Union-Find.
/// Returns sets of nodes, where each set represents an isolated connected subgraph.
pub fn weakly_connected_components(graph: &Graph) -> Vec<HashSet<NodeId>> {
    let mut ds = DisjointSet::new();

    for node in graph.nodes() {
        ds.make_set(node.id);
    }

    for edge in graph.edges() {
        ds.union(edge.source, edge.target);
    }

    let mut component_map: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
    for node in graph.nodes() {
        let root = ds.find(node.id);
        component_map.entry(root).or_default().insert(node.id);
    }

    component_map.into_values().collect()
}

/// Detect Strongly Connected Components (SCC) using Tarjan's algorithm.
pub fn strongly_connected_components(graph: &Graph) -> Vec<HashSet<NodeId>> {
    struct Tarjan<'a> {
        graph: &'a Graph,
        index: usize,
        indices: HashMap<NodeId, usize>,
        lowlink: HashMap<NodeId, usize>,
        on_stack: HashSet<NodeId>,
        stack: Vec<NodeId>,
        sccs: Vec<HashSet<NodeId>>,
    }

    impl<'a> Tarjan<'a> {
        fn strongconnect(&mut self, v: NodeId) {
            self.indices.insert(v, self.index);
            self.lowlink.insert(v, self.index);
            self.index += 1;
            self.stack.push(v);
            self.on_stack.insert(v);

            if let Ok(neighbors) = self.graph.neighbors(v, Direction::Outgoing) {
                for w in neighbors {
                    if !self.indices.contains_key(&w) {
                        self.strongconnect(w);
                        let w_low = self.lowlink[&w];
                        let v_low = self.lowlink.get_mut(&v).unwrap();
                        *v_low = (*v_low).min(w_low);
                    } else if self.on_stack.contains(&w) {
                        let w_idx = self.indices[&w];
                        let v_low = self.lowlink.get_mut(&v).unwrap();
                        *v_low = (*v_low).min(w_idx);
                    }
                }
            }

            if self.lowlink[&v] == self.indices[&v] {
                let mut component = HashSet::new();
                while let Some(w) = self.stack.pop() {
                    self.on_stack.remove(&w);
                    component.insert(w);
                    if w == v {
                        break;
                    }
                }
                self.sccs.push(component);
            }
        }
    }

    let mut tarjan = Tarjan {
        graph,
        index: 0,
        indices: HashMap::new(),
        lowlink: HashMap::new(),
        on_stack: HashSet::new(),
        stack: Vec::new(),
        sccs: Vec::new(),
    };

    for node in graph.nodes() {
        if !tarjan.indices.contains_key(&node.id) {
            tarjan.strongconnect(node.id);
        }
    }

    tarjan.sccs
}

/// Compute a topological ordering of nodes for a Directed Acyclic Graph (DAG) using Kahn's algorithm.
/// Returns an error if the graph contains a directed cycle.
pub fn topological_sort(graph: &Graph) -> Result<Vec<NodeId>> {
    let mut in_degrees: HashMap<NodeId, usize> = HashMap::with_capacity(graph.node_count());
    let mut queue = VecDeque::new();

    for node in graph.nodes() {
        let deg = graph.degree(node.id, Direction::Incoming).unwrap_or(0);
        in_degrees.insert(node.id, deg);
        if deg == 0 {
            queue.push_back(node.id);
        }
    }

    let mut order = Vec::with_capacity(graph.node_count());

    while let Some(u) = queue.pop_front() {
        order.push(u);

        if let Ok(neighbors) = graph.neighbors(u, Direction::Outgoing) {
            for v in neighbors {
                if let Some(deg) = in_degrees.get_mut(&v) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }
    }

    if order.len() != graph.node_count() {
        return Err(GraphError::InvalidOperation(
            "Graph contains a directed cycle; topological sort is only defined for DAGs".into(),
        ));
    }

    Ok(order)
}
