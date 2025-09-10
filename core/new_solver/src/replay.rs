use std::collections::BTreeMap;

use hex::ToHex;
use pod2::{
    frontend::{MainPod, MainPodBuilder, Operation, OperationArg},
    middleware::{
        Hash, Key, OperationAux, OperationType, Params, Statement, StatementArg, VDSet, Value,
        ValueRef,
    },
};

use crate::{
    edb::EdbView,
    proof_dag::ProofDagWithOps,
    types::{ConstraintStore, OpTag},
};

/// Build a MainPod from a single engine answer by replaying its proof steps into frontend Operations.
///
/// - `input_pods`: known pods for CopyStatement provenance.
/// - `dicts`: known SignedDicts or Dictionaries by root for ContainsFromEntries and SignedBy.
/// - `public_selector`: marks which statements should be public (others are private).
pub fn build_pod_from_answer<F, G>(
    answer: &ConstraintStore,
    params: &Params,
    vd_set: &VDSet,
    prove_with: G,
    edb: &dyn EdbView,
    public_selector: F,
) -> Result<MainPod, String>
where
    F: Fn(&Statement) -> bool,
    G: Fn(&MainPodBuilder) -> Result<MainPod, String>,
{
    let dag = ProofDagWithOps::from_store(answer);

    // Build quick edge lookups
    let mut heads_for_op: BTreeMap<String, String> = BTreeMap::new();
    let mut premises_for_op: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (from, to) in dag.edges.iter() {
        if is_op_key(to) && is_stmt_key(from) {
            premises_for_op
                .entry(to.clone())
                .or_default()
                .push(from.clone());
        }
        if is_op_key(from) && is_stmt_key(to) {
            heads_for_op.insert(from.clone(), to.clone());
        }
    }
    // Stable order premises list
    for v in premises_for_op.values_mut() {
        v.sort();
    }

    let mut builder = MainPodBuilder::new(params, vd_set);
    // Resolve required input pods from the EDB using the answer's provenance
    let required = answer.required_pods();
    if required.len() > params.max_input_pods {
        return Err(format!(
            "replay requires {} input pods; exceeds max_input_pods {}",
            required.len(),
            params.max_input_pods
        ));
    }
    for r in required.iter() {
        let pod = edb.resolve_pod(r).ok_or_else(|| {
            format!(
                "missing input pod for ref: 0x{}",
                r.0.encode_hex::<String>()
            )
        })?;
        builder.add_pod(pod);
    }

    // Build op dependency graph: producer_op -> consumer_op if consumer uses a statement produced by producer
    let mut stmt_producers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (from, to) in dag.edges.iter() {
        if is_op_key(from) && is_stmt_key(to) {
            stmt_producers
                .entry(to.clone())
                .or_default()
                .push(from.clone());
        }
    }
    // adjacency over ops
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let all_ops: Vec<String> = dag.op_nodes.keys().cloned().collect();
    for op_key in all_ops.iter() {
        let mut outs: Vec<String> = Vec::new();
        if let Some(prem_keys) = premises_for_op.get(op_key) {
            for pk in prem_keys.iter() {
                if let Some(prods) = stmt_producers.get(pk) {
                    for prod in prods.iter() {
                        if prod != op_key {
                            outs.push(op_key.clone()); // placeholder, will fill below
                        }
                    }
                }
            }
        }
        let _ = outs; // suppress unused (we build adj below)
    }
    // Build edges: for each consumer op, add edges from each producer of its premise statements
    for (consumer, prem_keys) in premises_for_op.iter() {
        for pk in prem_keys.iter() {
            if let Some(prods) = stmt_producers.get(pk) {
                for prod in prods.iter() {
                    if prod != consumer {
                        adj.entry(prod.clone()).or_default().push(consumer.clone());
                    }
                }
            }
        }
    }
    for v in adj.values_mut() {
        v.sort();
        v.dedup();
    }
    // Kahn topo sort over op keys
    let mut indeg: BTreeMap<String, usize> = BTreeMap::new();
    for k in dag.op_nodes.keys() {
        indeg.insert(k.clone(), 0);
    }
    for (_from, tos) in adj.iter() {
        for to in tos.iter() {
            if let Some(d) = indeg.get_mut(to) {
                *d = d.saturating_add(1);
            }
        }
    }
    let mut queue: std::collections::VecDeque<String> = indeg
        .iter()
        .filter_map(|(k, &d)| if d == 0 { Some(k.clone()) } else { None })
        .collect();
    let mut topo_ops: Vec<String> = Vec::new();
    while let Some(k) = queue.pop_front() {
        topo_ops.push(k.clone());
        if let Some(nei) = adj.get(&k) {
            for to in nei.iter() {
                if let Some(d) = indeg.get_mut(to) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push_back(to.clone());
                    }
                }
            }
        }
    }
    if topo_ops.len() < dag.op_nodes.len() {
        // Fallback: append any remaining ops in stable key order
        let mut remaining: Vec<String> = dag
            .op_nodes
            .keys()
            .filter(|k| !topo_ops.contains(*k))
            .cloned()
            .collect();
        remaining.sort();
        topo_ops.extend(remaining);
    }

    // Emit operations following topological order
    let mut inserted_ops: usize = 0;
    for op_key in topo_ops.into_iter() {
        let tag = match dag.op_nodes.get(&op_key) {
            Some(t) => t,
            None => continue,
        };
        let head_key = match heads_for_op.get(&op_key) {
            Some(k) => k,
            None => continue,
        };
        let head_stmt = dag
            .stmt_nodes
            .get(head_key)
            .ok_or_else(|| "broken DAG: missing head statement".to_string())?;
        let premise_stmts: Vec<&Statement> = premises_for_op
            .get(&op_key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|k| dag.stmt_nodes.get(&k))
            .collect();

        // Map (tag, head, premises) -> frontend Operation
        if let Some(op) = map_to_operation(tag, head_stmt, &premise_stmts, edb)? {
            if inserted_ops + 1 > params.max_statements {
                return Err(format!(
                    "replay requires {} operations; exceeds max_statements {}",
                    inserted_ops + 1,
                    params.max_statements
                ));
            }
            let public = public_selector(head_stmt);
            // Insert operation as private to ensure an earlier source for public copies,
            // then mark as public if selected.
            let st = builder.priv_op(op).map_err(|e| e.to_string())?;
            inserted_ops += 1;
            if public {
                builder.reveal(&st);
            }
        } else {
            // Even if we skip emitting an op (e.g., CopyStatement), still mark as public if selected
            if public_selector(head_stmt) {
                builder.reveal(head_stmt);
            }
        }
    }

    prove_with(&builder)
}

fn is_op_key(k: &str) -> bool {
    k.starts_with("O|")
}
fn is_stmt_key(k: &str) -> bool {
    k.starts_with("S|")
}

/// Compute a selector that marks only "top-level" statements as public.
/// Top-level = head statements that are not used as premises to any later operation.
pub fn top_level_public_selector(answer: &ConstraintStore) -> impl Fn(&Statement) -> bool {
    use std::collections::BTreeSet;
    let dag = ProofDagWithOps::from_store(answer);
    let mut premise_stmt_keys: BTreeSet<String> = BTreeSet::new();
    let mut head_stmt_keys: BTreeSet<String> = BTreeSet::new();
    for (from, to) in dag.edges.iter() {
        if is_stmt_key(from) && is_op_key(to) {
            premise_stmt_keys.insert(from.clone());
        }
        if is_op_key(from) && is_stmt_key(to) {
            head_stmt_keys.insert(to.clone());
        }
    }
    let top: BTreeSet<String> = head_stmt_keys
        .difference(&premise_stmt_keys)
        .cloned()
        .collect();
    move |st: &Statement| {
        let key = format!("S|{}", canonical_stmt_key(st));
        top.contains(&key)
    }
}

/// Wrapper that builds a POD with a policy where only top-level statements are public.
pub fn build_pod_from_answer_top_level_public<G>(
    answer: &ConstraintStore,
    params: &Params,
    vd_set: &VDSet,
    prove_with: G,
    edb: &dyn EdbView,
) -> Result<MainPod, String>
where
    G: Fn(&MainPodBuilder) -> Result<MainPod, String>,
{
    let selector = top_level_public_selector(answer);
    build_pod_from_answer(answer, params, vd_set, prove_with, edb, selector)
}

fn canonical_stmt_key(st: &Statement) -> String {
    use hex::ToHex;
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

fn map_to_operation(
    tag: &OpTag,
    head: &Statement,
    premises: &[&Statement],
    edb: &dyn EdbView,
) -> Result<Option<Operation>, String> {
    use pod2::middleware::{NativeOperation, Predicate};

    // Skip emitting private Copy operations; rely on public Copy from input pods or other proofs
    if let OpTag::CopyStatement { .. } = tag {
        return Ok(None);
    }

    match head.predicate() {
        Predicate::Custom(cpr) => match tag {
            OpTag::CustomDeduction { .. } => {
                // Order and normalize premises to match the predicate's template order
                let ordered = order_custom_premises(&cpr, head, premises, edb)?;
                let mut args: Vec<Statement> = Vec::new();
                for s in ordered.into_iter() {
                    // Normalize Contains root to full dict values when possible; preserve None placeholders
                    if let Statement::None = s {
                        args.push(s);
                    } else {
                        args.push(normalize_stmt_for_op_arg(s, edb)?);
                    }
                }
                Ok(Some(Operation::custom(cpr.clone(), args)))
            }
            _ => Ok(None),
        },
        Predicate::Native(np) => {
            use pod2::middleware::NativePredicate::*;
            match np {
                // Value-centric natives: translate AKs to Contains statements from premises
                Equal | Lt | LtEq | NotEqual => {
                    let (l, r, op) = match head.clone() {
                        Statement::Equal(l, r) => (l, r, NativeOperation::EqualFromEntries),
                        Statement::Lt(l, r) => (l, r, NativeOperation::LtFromEntries),
                        Statement::LtEq(l, r) => (l, r, NativeOperation::LtEqFromEntries),
                        Statement::NotEqual(l, r) => (l, r, NativeOperation::NotEqualFromEntries),
                        _ => unreachable!(),
                    };
                    let a0 = op_arg_from_vr(l, premises, edb)?;
                    let a1 = op_arg_from_vr(r, premises, edb)?;
                    Ok(Some(Operation(
                        OperationType::Native(op),
                        vec![a0, a1],
                        OperationAux::None,
                    )))
                }
                SumOf | ProductOf | MaxOf | HashOf => {
                    let (a, b, c, op) = match head.clone() {
                        Statement::SumOf(a, b, c) => (a, b, c, NativeOperation::SumOf),
                        Statement::ProductOf(a, b, c) => (a, b, c, NativeOperation::ProductOf),
                        Statement::MaxOf(a, b, c) => (a, b, c, NativeOperation::MaxOf),
                        Statement::HashOf(a, b, c) => (a, b, c, NativeOperation::HashOf),
                        _ => unreachable!(),
                    };
                    let a0 = op_arg_from_vr(a, premises, edb)?;
                    let a1 = op_arg_from_vr(b, premises, edb)?;
                    let a2 = op_arg_from_vr(c, premises, edb)?;
                    Ok(Some(Operation(
                        OperationType::Native(op),
                        vec![a0, a1, a2],
                        OperationAux::None,
                    )))
                }
                PublicKeyOf => {
                    let (a, b, op) = match head.clone() {
                        Statement::PublicKeyOf(a, b) => (a, b, NativeOperation::PublicKeyOf),
                        _ => unreachable!(),
                    };
                    let a0 = op_arg_from_vr(a, premises, edb)?;
                    let a1 = op_arg_from_vr(b, premises, edb)?;
                    Ok(Some(Operation(
                        OperationType::Native(op),
                        vec![a0, a1],
                        OperationAux::None,
                    )))
                }
                Contains => {
                    // If this was generated from a full dict, emit ContainsFromEntries using the dict value
                    if let OpTag::GeneratedContains { root, .. } = tag {
                        if let Some(dict) = edb.full_dict(root) {
                            if let Statement::Contains(_r, k, v) = head.clone() {
                                // Expect k and v to be literals here
                                if let (ValueRef::Literal(kv), ValueRef::Literal(vv)) = (k, v) {
                                    return Ok(Some(Operation(
                                        OperationType::Native(NativeOperation::ContainsFromEntries),
                                        vec![
                                            OperationArg::from(Value::from(dict)),
                                            OperationArg::from(kv),
                                            OperationArg::from(vv),
                                        ],
                                        OperationAux::None,
                                    )));
                                }
                            }
                        } else {
                            return Err("missing dictionary for GeneratedContains; cannot replay"
                                .to_string());
                        }
                    }
                    Ok(Some(Operation::copy(head.clone())))
                }
                NotContains => {
                    // This predicate can be proven from a literal container OR from a full dictionary
                    // in the EDB. In both cases, the MainPod operation is NotContainsFromEntries.
                    if let Statement::NotContains(r, k) = head.clone() {
                        if let (ValueRef::Literal(vr), ValueRef::Literal(kv)) = (r, k) {
                            // First, check if the value is already a literal container.
                            if let pod2::middleware::TypedValue::Dictionary(_)
                            | pod2::middleware::TypedValue::Array(_)
                            | pod2::middleware::TypedValue::Set(_) = vr.typed()
                            {
                                return Ok(Some(Operation(
                                    OperationType::Native(NativeOperation::NotContainsFromEntries),
                                    vec![OperationArg::from(vr), OperationArg::from(kv)],
                                    OperationAux::None,
                                )));
                            }

                            // If not, it's a hash; try to look up the full dictionary in the EDB.
                            let root = Hash::from(vr.raw());
                            if let Some(dict) = edb.full_dict(&root) {
                                return Ok(Some(Operation(
                                    OperationType::Native(NativeOperation::NotContainsFromEntries),
                                    vec![
                                        OperationArg::from(Value::from(dict)),
                                        OperationArg::from(kv),
                                    ],
                                    OperationAux::None,
                                )));
                            }
                        }
                    }
                    // If neither of the above, it must be a copied statement.
                    Ok(Some(Operation::copy(head.clone())))
                }
                SignedBy => {
                    if let Statement::SignedBy(ValueRef::Literal(msg), v_pk) = head.clone() {
                        let root = Hash::from(msg.raw());
                        let pk = op_arg_from_vr(v_pk, premises, edb)?;
                        if let Some(sd) = edb.signed_dict(&root) {
                            return Ok(Some(Operation::signed_by(root, pk, sd.signature.clone())));
                        } else {
                            return Err(
                                "missing SignedDict for SignedBy; cannot replay".to_string()
                            );
                        }
                    }
                    Err("SignedBy expects literal message root".to_string())
                }
                // TODO: Container update predicates should be supported
                None | False | ContainerInsert | ContainerDelete | ContainerUpdate
                | DictContains | DictNotContains | SetContains | SetNotContains | ArrayContains
                | GtEq | Gt | DictInsert | DictUpdate | DictDelete | SetInsert | SetDelete
                | ArrayUpdate => Ok(std::option::Option::None),
            }
        }
        _ => Ok(None),
    }
}

fn find_contains_for_ak(
    ak: &pod2::middleware::AnchoredKey,
    premises: &[&Statement],
) -> Option<Statement> {
    for s in premises.iter() {
        if let Statement::Contains(
            ValueRef::Literal(r),
            ValueRef::Literal(kv),
            ValueRef::Literal(_v),
        ) = s
        {
            if let Ok(kstr) = String::try_from(kv.typed()) {
                if Hash::from(r.raw()) == ak.root && Key::from(kstr) == ak.key {
                    return Some((*s).clone());
                }
            }
        }
    }
    None
}

fn op_arg_from_vr(
    vr: ValueRef,
    premises: &[&Statement],
    edb: &dyn EdbView,
) -> Result<OperationArg, String> {
    match vr {
        ValueRef::Literal(v) => Ok(OperationArg::from(v)),
        ValueRef::Key(ak) => {
            let c = find_contains_for_ak(&ak, premises)
                .ok_or_else(|| "missing Contains premise for anchored key argument".to_string())?;
            // Normalize first arg to a full dictionary value when available to avoid builder auto-dict_contains on roots
            let c_norm = match c.clone() {
                Statement::Contains(ValueRef::Literal(r), k, v) => {
                    let root = Hash::from(r.raw());
                    if let Some(dict) = edb.full_dict(&root) {
                        Statement::Contains(ValueRef::Literal(Value::from(dict)), k, v)
                    } else {
                        return Err(
                            "missing full dictionary for anchored key argument; cannot replay"
                                .to_string(),
                        );
                    }
                }
                _ => c,
            };
            Ok(OperationArg::from(c_norm))
        }
    }
}

fn normalize_stmt_for_op_arg(s: Statement, edb: &dyn EdbView) -> Result<Statement, String> {
    use pod2::middleware::TypedValue;
    match s.clone() {
        Statement::Contains(ValueRef::Literal(r), k, v) => {
            // If the value is already a container literal, it's "normalized" and doesn't need EDB lookup.
            if let TypedValue::Dictionary(_) | TypedValue::Array(_) | TypedValue::Set(_) = r.typed()
            {
                return Ok(s);
            }

            // Otherwise, assume it's a root hash and look it up in the EDB.
            let root = Hash::from(r.raw());
            if let Some(dict) = edb.full_dict(&root) {
                Ok(Statement::Contains(
                    ValueRef::Literal(Value::from(dict)),
                    k,
                    v,
                ))
            } else {
                Err(
                    "missing full dictionary for Contains premise in custom op; cannot replay"
                        .to_string(),
                )
            }
        }
        _ => Ok(s),
    }
}

fn order_custom_premises(
    cpr: &pod2::middleware::CustomPredicateRef,
    head_stmt: &Statement,
    premises: &[&Statement],
    edb: &dyn EdbView,
) -> Result<Vec<Statement>, String> {
    use pod2::middleware::{
        NativePredicate as NP, Predicate, Statement as Stmt, StatementTmpl,
        StatementTmplArg as STA, ValueRef as VR,
    };
    let templates: Vec<StatementTmpl> = cpr.predicate().statements().to_vec();
    let args_len: usize = cpr.predicate().args_len();
    let head_vals: Option<Vec<Value>> = match head_stmt.clone() {
        Statement::Custom(_, vals) => Some(vals),
        _ => None,
    };
    let mut out: Vec<Statement> = Vec::with_capacity(templates.len());
    let mut available_premises: Vec<&Statement> = premises.to_vec();
    // Build a human-friendly inventory of available premises for debugging
    let inventory = premises
        .iter()
        .enumerate()
        .map(|(i, s)| format!("#{i}: {}", describe_stmt(s)))
        .collect::<Vec<_>>()
        .join("\n");
    for tmpl in templates.iter() {
        // Helper: compare template arg against candidate ValueRef using head bindings/literals
        let arg_matches = |targ: &STA, carg: &VR| -> bool {
            match targ {
                STA::Literal(vlit) => match carg {
                    VR::Literal(v) => v.raw() == vlit.raw(),
                    _ => false,
                },
                STA::Wildcard(w) => {
                    if w.index < args_len {
                        if let Some(hv) = head_vals.as_ref().and_then(|hv| hv.get(w.index)) {
                            match carg {
                                VR::Literal(v) => v.raw() == hv.raw(),
                                _ => false,
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                }
                STA::AnchoredKey(w, key) => match carg {
                    VR::Key(ak) => {
                        if ak.key.name() != key.name() {
                            return false;
                        }
                        if w.index < args_len {
                            if let Some(hv) = head_vals.as_ref().and_then(|hv| hv.get(w.index)) {
                                return ak.root == pod2::middleware::Hash::from(hv.raw());
                            }
                        }
                        true
                    }
                    _ => false,
                },
                STA::None => true,
            }
        };

        // Find a premise that matches this template's predicate and constraints
        let matched_pos =
            available_premises
                .iter()
                .position(|s| match (tmpl.pred(), (**s).clone()) {
                    (Predicate::Native(NP::Contains), Stmt::Contains(a0, a1, a2)) => {
                        let args = tmpl.args();
                        arg_matches(&args[0], &a0)
                            && arg_matches(&args[1], &a1)
                            && arg_matches(&args[2], &a2)
                    }
                    (Predicate::BatchSelf(i), Stmt::Custom(sub_cpr, sub_args)) => {
                        if !(sub_cpr.batch == cpr.batch && sub_cpr.index == *i) {
                            return false;
                        }
                        // Enforce head-projected args where template uses head wildcards
                        let targs = tmpl.args();
                        for (pos, targ) in targs.iter().enumerate() {
                            if let STA::Wildcard(w) = targ {
                                if w.index < args_len {
                                    if let Some(hv) =
                                        head_vals.as_ref().and_then(|hv| hv.get(w.index))
                                    {
                                        if let Some(v) = sub_args.get(pos) {
                                            if v.raw() != hv.raw() {
                                                return false;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        true
                    }
                    (Predicate::Custom(exp_cpr), Stmt::Custom(sub_cpr, _)) => sub_cpr == *exp_cpr,
                    (Predicate::Native(NP::SignedBy), Stmt::SignedBy(a0, a1)) => {
                        let args = tmpl.args();
                        arg_matches(&args[0], &a0) && arg_matches(&args[1], &a1)
                    }
                    (Predicate::Native(NP::Equal), Stmt::Equal(a0, a1)) => {
                        let args = tmpl.args();
                        arg_matches(&args[0], &a0) && arg_matches(&args[1], &a1)
                    }
                    (Predicate::Native(NP::Lt), Stmt::Lt(a0, a1)) => {
                        let args = tmpl.args();
                        arg_matches(&args[0], &a0) && arg_matches(&args[1], &a1)
                    }
                    (Predicate::Native(NP::LtEq), Stmt::LtEq(a0, a1)) => {
                        let args = tmpl.args();
                        arg_matches(&args[0], &a0) && arg_matches(&args[1], &a1)
                    }
                    (Predicate::Native(NP::SumOf), Stmt::SumOf(a0, a1, a2)) => {
                        let args = tmpl.args();
                        arg_matches(&args[0], &a0)
                            && arg_matches(&args[1], &a1)
                            && arg_matches(&args[2], &a2)
                    }
                    _ => false,
                });
        if let Some(pos) = matched_pos {
            let s = available_premises.remove(pos);
            out.push(normalize_stmt_for_op_arg(s.clone(), edb)?);
        } else {
            // For OR predicates compiled as BatchSelf(i) entries, we must supply Statement::None
            // for branches that did not fire. For all other predicates, fail loudly.
            match tmpl.pred() {
                pod2::middleware::Predicate::BatchSelf(_)
                | pod2::middleware::Predicate::Custom(_) => {
                    out.push(Statement::None);
                }
                _ => {
                    return Err(format!(
                        "missing premise matching template {:?}\nAvailable premises:\n{}",
                        tmpl.pred(),
                        inventory
                    ));
                }
            }
        }
    }
    Ok(out)
}

fn describe_stmt(s: &Statement) -> String {
    use pod2::middleware::Statement as St;
    match s {
        St::Contains(a0, a1, a2) => format!(
            "Contains({}, {}, {})",
            describe_vr(a0),
            describe_vr(a1),
            describe_vr(a2)
        ),
        St::SignedBy(a0, a1) => format!("SignedBy({}, {})", describe_vr(a0), describe_vr(a1)),
        St::Equal(a0, a1) => format!("Equal({}, {})", describe_vr(a0), describe_vr(a1)),
        St::Lt(a0, a1) => format!("Lt({}, {})", describe_vr(a0), describe_vr(a1)),
        St::LtEq(a0, a1) => format!("LtEq({}, {})", describe_vr(a0), describe_vr(a1)),
        St::SumOf(a0, a1, a2) => format!(
            "SumOf({}, {}, {})",
            describe_vr(a0),
            describe_vr(a1),
            describe_vr(a2)
        ),
        St::Custom(cpr, _args) => format!("Custom({}:{})", cpr.predicate().name, cpr.index),
        other => format!("{:?}", other.predicate()),
    }
}

fn describe_vr(vr: &pod2::middleware::ValueRef) -> String {
    use pod2::middleware::ValueRef as VR;
    match vr {
        VR::Literal(v) => format!("{v}"),
        VR::Key(ak) => format!("{}[\"{}\"]", ak.root, ak.key.name()),
    }
}
