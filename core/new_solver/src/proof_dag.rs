use std::collections::{BTreeMap, BTreeSet, VecDeque};

use hex::ToHex;
use pod2::middleware::{Statement, StatementArg};

use crate::types::{ConstraintStore, OpTag};

/// A simple, deterministic DAG of proof steps where nodes are concrete Statements
/// and edges go from premise -> head (the statement that the premise helps prove).
#[derive(Clone, Debug, Default)]
pub struct ProofDag {
    /// Canonical key -> Statement
    pub nodes: BTreeMap<String, Statement>,
    /// Edge set stored as canonical (from_key, to_key)
    pub edges: BTreeSet<(String, String)>,
}

impl ProofDag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a proof DAG from a `ConstraintStore`'s premises, following nested
    /// Derived/CustomDeduction premises recursively.
    pub fn from_store(store: &ConstraintStore) -> Self {
        let mut dag = ProofDag::new();
        // Use a queue to avoid deep recursion; push all top-level (stmt, tag) pairs
        let mut work: Vec<(Statement, OpTag)> = store.premises.clone();
        // Process breadth-first to register nodes early and add edges deterministically
        while let Some((head, tag)) = work.pop() {
            let head_key = canonical_stmt_key(&head);
            dag.nodes.entry(head_key.clone()).or_insert(head.clone());
            match tag {
                OpTag::Derived { premises } => {
                    for (pst, ptag) in premises.into_iter() {
                        let pkey = canonical_stmt_key(&pst);
                        dag.nodes.entry(pkey.clone()).or_insert(pst.clone());
                        dag.edges.insert((pkey.clone(), head_key.clone()));
                        // Recurse: push the premise pair to handle its own dependencies
                        work.push((pst, ptag));
                    }
                }
                OpTag::CustomDeduction { premises, .. } => {
                    for (pst, ptag) in premises.into_iter() {
                        let pkey = canonical_stmt_key(&pst);
                        dag.nodes.entry(pkey.clone()).or_insert(pst.clone());
                        dag.edges.insert((pkey.clone(), head_key.clone()));
                        work.push((pst, ptag));
                    }
                }
                OpTag::CopyStatement { .. }
                | OpTag::FromLiterals
                | OpTag::GeneratedContains { .. } => {
                    // Leaf; no extra edges
                }
            }
        }
        dag
    }

    /// Return a deterministic topological ordering of statements (premises before heads).
    /// If there are multiple valid orders, the order is stabilized by the canonical key ordering.
    pub fn topo_order(&self) -> Vec<Statement> {
        // Compute indegrees
        let mut indeg: BTreeMap<&String, usize> = self
            .nodes
            .keys()
            .map(|k| (k, 0usize))
            .collect::<BTreeMap<_, _>>();
        for (from, to) in self.edges.iter() {
            if let Some(e) = indeg.get_mut(to) {
                *e = e.saturating_add(1);
            }
            // Ensure from exists in indeg (it should, but be safe)
            indeg.entry(from).or_insert(0);
        }
        // Initialize queue with 0-indegree nodes (sorted by key)
        let mut queue: VecDeque<&String> = indeg
            .iter()
            .filter_map(|(k, &d)| if d == 0 { Some(*k) } else { None })
            .collect::<Vec<_>>()
            .into();

        // Build adjacency list for efficient traversal
        let mut adj: BTreeMap<&String, Vec<&String>> = BTreeMap::new();
        for (from, to) in self.edges.iter() {
            adj.entry(from).or_default().push(to);
        }
        for v in adj.values_mut() {
            v.sort();
        }

        let mut out: Vec<Statement> = Vec::new();
        // Stable visited to prevent duplicates if any stray node re-enters
        let mut seen_keys: BTreeSet<&String> = BTreeSet::new();
        while let Some(k) = queue.pop_front() {
            if !seen_keys.insert(k) {
                continue;
            }
            if let Some(st) = self.nodes.get(k) {
                out.push(st.clone());
            }
            if let Some(nei) = adj.get(k) {
                for &to in nei.iter() {
                    if let Some(d) = indeg.get_mut(to) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            queue.push_back(to);
                        }
                    }
                }
            }
        }
        out
    }

    /// Export the DAG as Graphviz DOT text.
    /// - Nodes are assigned stable IDs `n0`, `n1`, ... in canonical-key order.
    /// - Each node is labeled with the pretty-printed Statement.
    /// - rankdir=LR and monospace labels for readability.
    pub fn to_dot(&self) -> String {
        // Assign stable ids to keys
        let mut id_of: BTreeMap<&String, String> = BTreeMap::new();
        for (i, k) in self.nodes.keys().enumerate() {
            id_of.insert(k, format!("n{i}"));
        }
        let mut out = String::new();
        out.push_str("digraph ProofDag {\n");
        out.push_str("  rankdir=LR;\n  node [shape=box, fontname=\"monospace\", fontsize=10];\n");
        // Emit nodes
        for (k, st) in self.nodes.iter() {
            let id = id_of.get(k).unwrap();
            // Escape label quotes/backslashes minimally
            let mut label = format!("{st}");
            label = label.replace('\\', "\\\\").replace('"', "\\\"");
            let mut tooltip = k.clone();
            tooltip = tooltip.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "  {id} [label=\"{label}\", tooltip=\"{tooltip}\"];\n"
            ));
        }
        // Emit edges
        for (from, to) in self.edges.iter() {
            if let (Some(fid), Some(tid)) = (id_of.get(from), id_of.get(to)) {
                out.push_str(&format!("  {fid} -> {tid};\n"));
            }
        }
        out.push_str("}\n");
        out
    }
}

/// Canonical string key for a Statement: predicate + raw commitments of args.
fn canonical_stmt_key(st: &Statement) -> String {
    let mut s = String::new();
    s.push_str(&format!("{:?}|", st.predicate()));
    for arg in st.args().into_iter() {
        match arg {
            StatementArg::Literal(v) => {
                s.push_str(&v.raw().encode_hex::<String>());
                s.push('|');
            }
            StatementArg::Key(ak) => {
                s.push_str(&ak.root.encode_hex::<String>());
                s.push(':');
                s.push_str(ak.key.name());
                s.push('|');
            }
            StatementArg::None => s.push_str("none|"),
        }
    }
    s
}

/// Extended DAG that includes operation nodes and statement nodes separately,
/// with edges: premise_statement -> operation -> head_statement.
#[derive(Clone, Debug, Default)]
pub struct ProofDagWithOps {
    /// key -> Statement (keys prefixed with "S|")
    pub stmt_nodes: BTreeMap<String, Statement>,
    /// key -> OpTag (keys prefixed with "O|")
    pub op_nodes: BTreeMap<String, OpTag>,
    /// edges over keys (statement or op keys)
    pub edges: BTreeSet<(String, String)>,
}

impl ProofDagWithOps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the op-augmented DAG from a `ConstraintStore`.
    pub fn from_store(store: &ConstraintStore) -> Self {
        let mut dag = ProofDagWithOps::new();
        let mut work: Vec<(Statement, OpTag)> = store.premises.clone();
        while let Some((head, tag)) = work.pop() {
            let head_skey = format!("S|{}", canonical_stmt_key(&head));
            dag.stmt_nodes
                .entry(head_skey.clone())
                .or_insert(head.clone());

            // Always create an op node, even for leaves, to represent the proof step
            let op_key = format!("O|{}|{}", &head_skey, short_op_key(&tag));
            dag.op_nodes.entry(op_key.clone()).or_insert(tag.clone());
            // op -> head statement
            dag.edges.insert((op_key.clone(), head_skey.clone()));

            match tag {
                OpTag::Derived { premises } => {
                    for (pst, ptag) in premises.into_iter() {
                        let p_skey = format!("S|{}", canonical_stmt_key(&pst));
                        dag.stmt_nodes.entry(p_skey.clone()).or_insert(pst.clone());
                        // premise -> op
                        dag.edges.insert((p_skey.clone(), op_key.clone()));
                        work.push((pst, ptag));
                    }
                }
                OpTag::CustomDeduction { premises, .. } => {
                    for (pst, ptag) in premises.into_iter() {
                        let p_skey = format!("S|{}", canonical_stmt_key(&pst));
                        dag.stmt_nodes.entry(p_skey.clone()).or_insert(pst.clone());
                        dag.edges.insert((p_skey.clone(), op_key.clone()));
                        work.push((pst, ptag));
                    }
                }
                OpTag::CopyStatement { .. }
                | OpTag::FromLiterals
                | OpTag::GeneratedContains { .. } => {
                    // Leaves: no premise statements to attach
                }
            }
        }
        dag
    }

    /// Export to Graphviz DOT with separate shapes:
    /// - Statements: shape=box
    /// - Operations: shape=ellipse
    pub fn to_dot(&self) -> String {
        // Combine keys for stable ID assignment
        let mut all_keys: Vec<&String> = Vec::new();
        all_keys.extend(self.stmt_nodes.keys());
        all_keys.extend(self.op_nodes.keys());
        all_keys.sort();
        let mut id_of: BTreeMap<&String, String> = BTreeMap::new();
        for (i, k) in all_keys.iter().enumerate() {
            id_of.insert(*k, format!("n{i}"));
        }
        let mut out = String::new();
        out.push_str("digraph ProofDag {\n");
        out.push_str("  rankdir=LR;\n  node [fontname=\"monospace\", fontsize=10];\n");
        // Statement nodes
        for (k, st) in self.stmt_nodes.iter() {
            let id = id_of.get(k).unwrap();
            let mut label = format!("{st}");
            label = label.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!("  {id} [shape=box, label=\"{label}\"];\n"));
        }
        // Operation nodes
        for (k, tag) in self.op_nodes.iter() {
            let id = id_of.get(k).unwrap();
            let mut label = short_op_label(tag);
            label = label.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!("  {id} [shape=ellipse, label=\"{label}\"];\n"));
        }
        // Edges
        for (from, to) in self.edges.iter() {
            if let (Some(fid), Some(tid)) = (id_of.get(from), id_of.get(to)) {
                out.push_str(&format!("  {fid} -> {tid};\n"));
            }
        }
        out.push_str("}\n");
        out
    }

    /// Render a simple indented tree view as text, starting from top-level statements.
    /// Top-level statements are those that are not used as premises of any operation
    /// (i.e., they have no outgoing edges statement -> op). For each statement, its
    /// producing operation is printed indented by 2 spaces beneath it, and the
    /// operation's input statements are printed 2 spaces further, recursively.
    pub fn to_tree_text(&self) -> String {
        // Build quick lookup maps for traversal
        let mut stmt_to_ops: BTreeMap<&String, Vec<&String>> = BTreeMap::new(); // premise stmt -> [op]
        let mut op_to_stmts: BTreeMap<&String, Vec<&String>> = BTreeMap::new(); // op -> [premise stmt]

        for (from, to) in self.edges.iter() {
            match (
                self.stmt_nodes.get(from),
                self.op_nodes.get(from),
                self.stmt_nodes.get(to),
                self.op_nodes.get(to),
            ) {
                // premise statement -> operation
                (Some(_), None, None, Some(_)) => {
                    stmt_to_ops.entry(from).or_default().push(to);
                    op_to_stmts.entry(to).or_default().push(from);
                }
                // operation -> head statement
                (None, Some(_), Some(_), None) => {}
                _ => {}
            }
        }

        // Identify top-level statements: those with no outgoing edge stmt -> op
        let mut top_level_stmt_keys: Vec<&String> = self
            .stmt_nodes
            .keys()
            .filter(|k| !stmt_to_ops.contains_key(*k))
            .collect();
        top_level_stmt_keys.sort();

        // Helper to append indentation
        fn indent(out: &mut String, spaces: usize) {
            for _ in 0..spaces {
                out.push(' ');
            }
        }

        // Recursive printer: statement -> its producer op -> its premise statements ...
        fn write_stmt(
            dag: &ProofDagWithOps,
            stmt_key: &String,
            op_to_stmts: &BTreeMap<&String, Vec<&String>>,
            indent_spaces: usize,
            out: &mut String,
        ) {
            if let Some(st) = dag.stmt_nodes.get(stmt_key) {
                indent(out, indent_spaces);
                out.push_str(&format!("{st}\n"));
                // For each op that produces this statement (edge op -> stmt)
                // Find ops by scanning inverse mapping of op_to_head_stmt (avoid recomputing: derive from edges again)
                // Simpler: scan dag.edges for (op -> this stmt)
                let mut producing_ops: Vec<&String> = dag
                    .edges
                    .iter()
                    .filter_map(|(from, to)| {
                        if to == stmt_key && dag.op_nodes.contains_key(from) {
                            Some(from)
                        } else {
                            None
                        }
                    })
                    .collect();
                producing_ops.sort();
                for op_key in producing_ops.into_iter() {
                    if let Some(tag) = dag.op_nodes.get(op_key) {
                        indent(out, indent_spaces + 2);
                        out.push_str(&format!("{}\n", short_op_label(tag)));
                        // Premise statements for this op
                        let mut inputs: Vec<&String> =
                            op_to_stmts.get(op_key).cloned().unwrap_or_default();
                        inputs.sort();
                        for prem_key in inputs.into_iter() {
                            write_stmt(dag, prem_key, op_to_stmts, indent_spaces + 4, out);
                        }
                    }
                }
            }
        }

        let mut out = String::new();
        for sk in top_level_stmt_keys.into_iter() {
            write_stmt(self, sk, &op_to_stmts, 0, &mut out);
        }
        out
    }
}

fn short_op_key(tag: &OpTag) -> String {
    match tag {
        OpTag::CopyStatement { source } => format!("copy:{}", source.0.encode_hex::<String>()),
        OpTag::FromLiterals => "from_literals".to_string(),
        OpTag::GeneratedContains { root, key, value } => format!(
            "gen_contains:{}:{}:{}",
            root.encode_hex::<String>(),
            key.name(),
            value.raw().encode_hex::<String>()
        ),
        OpTag::Derived { .. } => "derived".to_string(),
        OpTag::CustomDeduction { rule_id, .. } => format!("custom:{rule_id:?}"),
    }
}

fn short_op_label(tag: &OpTag) -> String {
    match tag {
        OpTag::CopyStatement { source } => {
            format!(
                "CopyStatement\nsource=0x{}",
                source.0.encode_hex::<String>()
            )
        }
        OpTag::FromLiterals => "FromLiterals".to_string(),
        OpTag::GeneratedContains { root, key, value } => format!(
            "GeneratedContains\nroot=0x{}\\nkey={}\\nvalue={}",
            root.encode_hex::<String>(),
            key.name(),
            value
        ),
        OpTag::Derived { .. } => "Derived".to_string(),
        OpTag::CustomDeduction { rule_id, .. } => {
            format!("CustomDeduction: {}", rule_id.predicate().name)
        }
    }
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{Value, ValueRef};

    use super::*;

    #[test]
    fn proof_dag_builds_and_toposorts() {
        // Build a tiny proof: s_head derived from s_a and s_b
        let s_a = Statement::SumOf(
            ValueRef::Literal(Value::from(3)),
            ValueRef::Literal(Value::from(2)),
            ValueRef::Literal(Value::from(1)),
        );
        let s_b = Statement::Lt(
            ValueRef::Literal(Value::from(1)),
            ValueRef::Literal(Value::from(2)),
        );
        let s_head = Statement::Equal(
            ValueRef::Literal(Value::from(3)),
            ValueRef::Literal(Value::from(3)),
        );
        let mut store = ConstraintStore::default();
        store.premises.push((
            s_head.clone(),
            OpTag::Derived {
                premises: vec![
                    (s_a.clone(), OpTag::FromLiterals),
                    (s_b.clone(), OpTag::FromLiterals),
                ],
            },
        ));
        let dag = ProofDag::from_store(&store);
        // Expect 3 nodes and 2 edges
        assert_eq!(dag.nodes.len(), 3);
        assert_eq!(dag.edges.len(), 2);
        let order = dag.topo_order();
        // Head should appear after premises in topo order
        let pos_head = order
            .iter()
            .position(|s| matches!(s, Statement::Equal(_, _)))
            .unwrap();
        let pos_a = order
            .iter()
            .position(|s| matches!(s, Statement::SumOf(_, _, _)))
            .unwrap();
        let pos_b = order
            .iter()
            .position(|s| matches!(s, Statement::Lt(_, _)))
            .unwrap();
        assert!(pos_a < pos_head);
        assert!(pos_b < pos_head);
    }

    #[test]
    fn proof_dag_to_dot_has_nodes_and_edges() {
        let s_a = Statement::SumOf(
            ValueRef::Literal(Value::from(3)),
            ValueRef::Literal(Value::from(2)),
            ValueRef::Literal(Value::from(1)),
        );
        let s_head = Statement::Equal(
            ValueRef::Literal(Value::from(3)),
            ValueRef::Literal(Value::from(3)),
        );
        let mut store = ConstraintStore::default();
        store.premises.push((
            s_head.clone(),
            OpTag::Derived {
                premises: vec![(s_a.clone(), OpTag::FromLiterals)],
            },
        ));
        let dag = ProofDag::from_store(&store);
        let dot = dag.to_dot();
        assert!(dot.contains("digraph ProofDag"));
        assert!(dot.contains("->"));
        assert!(dot.contains(&format!("{s_a}")));
        assert!(dot.contains(&format!("{s_head}")));
    }

    #[test]
    fn proof_dag_with_ops_wires_premises_op_head() {
        let s_a = Statement::SumOf(
            ValueRef::Literal(Value::from(3)),
            ValueRef::Literal(Value::from(2)),
            ValueRef::Literal(Value::from(1)),
        );
        let s_b = Statement::Lt(
            ValueRef::Literal(Value::from(1)),
            ValueRef::Literal(Value::from(2)),
        );
        let s_head = Statement::Equal(
            ValueRef::Literal(Value::from(3)),
            ValueRef::Literal(Value::from(3)),
        );
        let mut store = ConstraintStore::default();
        store.premises.push((
            s_head.clone(),
            OpTag::Derived {
                premises: vec![
                    (s_a.clone(), OpTag::FromLiterals),
                    (s_b.clone(), OpTag::FromLiterals),
                ],
            },
        ));
        let dag = ProofDagWithOps::from_store(&store);
        // Expect 3 statement nodes and 3 operation nodes
        assert_eq!(dag.stmt_nodes.len(), 3);
        assert_eq!(dag.op_nodes.len(), 3);
        // DOT contains op -> head edge and premise -> op edges
        let dot = dag.to_dot();
        assert!(dot.contains("digraph ProofDag"));
        // Find at least three edges '->'
        assert!(dot.matches("->").count() >= 3);
    }
}
