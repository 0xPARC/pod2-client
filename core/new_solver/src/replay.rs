use std::collections::{BTreeMap, HashMap};

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
    types::{ConstraintStore, OpTag, PodRef},
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
    input_pods: &HashMap<PodRef, MainPod>,
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
    // Add unique input pods referenced by CopyStatement tags
    for r in answer.input_pods.iter() {
        let pod = input_pods.get(r).ok_or_else(|| {
            format!(
                "missing input pod for ref: 0x{}",
                r.0.encode_hex::<String>()
            )
        })?;
        builder.add_pod(pod.clone());
    }

    // Emit operations for each op node (stable order)
    for (op_key, tag) in dag.op_nodes.iter() {
        let head_key = match heads_for_op.get(op_key) {
            Some(k) => k,
            None => continue,
        };
        let head_stmt = dag
            .stmt_nodes
            .get(head_key)
            .ok_or_else(|| "broken DAG: missing head statement".to_string())?;
        let premise_stmts: Vec<&Statement> = premises_for_op
            .get(op_key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|k| dag.stmt_nodes.get(&k))
            .collect();

        // Map (tag, head, premises) -> frontend Operation
        if let Some(op) = map_to_operation(tag, head_stmt, &premise_stmts, edb)? {
            let public = public_selector(head_stmt);
            // Insert operation; frontend validates
            if public {
                let _ = builder.pub_op(op).map_err(|e| e.to_string())?;
            } else {
                let _ = builder.priv_op(op).map_err(|e| e.to_string())?;
            }
        }
    }

    println!("builder: {builder}");

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
    input_pods: &HashMap<PodRef, MainPod>,
    edb: &dyn EdbView,
) -> Result<MainPod, String>
where
    G: Fn(&MainPodBuilder) -> Result<MainPod, String>,
{
    let selector = top_level_public_selector(answer);
    build_pod_from_answer(
        answer, params, vd_set, prove_with, input_pods, edb, selector,
    )
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

    // Copy stays copy regardless of predicate
    if let OpTag::CopyStatement { .. } = tag {
        return Ok(Some(Operation::copy(head.clone())));
    }

    match head.predicate() {
        Predicate::Custom(cpr) => match tag {
            OpTag::CustomDeduction {
                premises: ordered_body,
                ..
            } => {
                // Use the engine-provided ordered body directly (with None placeholders for OR)
                let mut args: Vec<Statement> = Vec::with_capacity(ordered_body.len());
                for (st, _t) in ordered_body.iter() {
                    args.push(normalize_stmt_for_op_arg(st.clone(), edb)?);
                }
                Ok(Some(Operation::custom(cpr.clone(), args)))
            }
            _ => Ok(None),
        },
        Predicate::Native(np) => {
            use pod2::middleware::NativePredicate::*;
            match np {
                // Value-centric natives: translate AKs to Contains statements from premises
                Equal | Lt | LtEq => {
                    let (l, r, op) = match head.clone() {
                        Statement::Equal(l, r) => (l, r, NativeOperation::EqualFromEntries),
                        Statement::Lt(l, r) => (l, r, NativeOperation::LtFromEntries),
                        Statement::LtEq(l, r) => (l, r, NativeOperation::LtEqFromEntries),
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
                SumOf => {
                    if let Statement::SumOf(a, b, c) = head.clone() {
                        let a0 = op_arg_from_vr(a, premises, edb)?;
                        let a1 = op_arg_from_vr(b, premises, edb)?;
                        let a2 = op_arg_from_vr(c, premises, edb)?;
                        Ok(Some(Operation(
                            OperationType::Native(NativeOperation::SumOf),
                            vec![a0, a1, a2],
                            OperationAux::None,
                        )))
                    } else {
                        Err("head not SumOf".to_string())
                    }
                }
                Contains => {
                    // If this was generated from a full dict, emit ContainsFromEntries using the dict value; else copy
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
                    // Our handler uses FromLiterals when full dict shows absence. Generate only if dict is present.
                    if let Statement::NotContains(r, k) = head.clone() {
                        if let (ValueRef::Literal(vr), ValueRef::Literal(kv)) = (r, k) {
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
                            } else {
                                return Err(
                                    "missing dictionary for NotContainsFromEntries; cannot replay"
                                        .to_string(),
                                );
                            }
                        }
                    }
                    Ok(Some(Operation::copy(head.clone())))
                }
                SignedBy => {
                    if let Statement::SignedBy(v_msg, _v_pk) = head.clone() {
                        if let ValueRef::Literal(msg) = v_msg {
                            let root = Hash::from(msg.raw());
                            if let Some(sd) = edb.signed_dict(&root) {
                                return Ok(Some(Operation::dict_signed_by(&sd)));
                            } else {
                                return Err(
                                    "missing SignedDict for SignedBy; cannot replay".to_string()
                                );
                            }
                        }
                    }
                    Err("SignedBy expects literal message root".to_string())
                }
                _ => Ok(::std::option::Option::None),
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

// removed: replaced by EdbView::full_dict

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
    match s.clone() {
        Statement::Contains(ValueRef::Literal(r), k, v) => {
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
    let mut out: Vec<Statement> = Vec::with_capacity(templates.len());
    // Build a human-friendly inventory of available premises for debugging
    let inventory = premises
        .iter()
        .enumerate()
        .map(|(i, s)| format!("#{i}: {}", describe_stmt(s)))
        .collect::<Vec<_>>()
        .join("\n");
    for tmpl in templates.iter() {
        // Find a premise that matches this template's predicate and any literal constraints
        let matched = premises.iter().find(|s| match (tmpl.pred(), (*s).clone()) {
            (Predicate::Native(np), Stmt::Contains(_, a1, _)) if matches!(np, NP::Contains) => {
                // If template's second arg is a literal string, enforce it
                match tmpl.args()[1] {
                    STA::Literal(ref v) => match (a1.clone(), String::try_from(v.typed()).ok()) {
                        (VR::Literal(kv_lit), Some(kstr)) => kv_lit == Value::from(kstr),
                        _ => true,
                    },
                    _ => true,
                }
            }
            (Predicate::BatchSelf(i), Stmt::Custom(sub_cpr, sub_args)) => {
                // Match subcall within the same batch by index
                if !(sub_cpr.batch == cpr.batch && sub_cpr.index == *i) {
                    return false;
                }
                // Additionally, require that subcall arguments align with the outer head where applicable
                if let Statement::Custom(parent_cpr, parent_args) = head_stmt.clone() {
                    if parent_cpr.batch == cpr.batch && parent_cpr.index == cpr.index {
                        // For OR parent eth_dos: BatchSelf(1) and (2) share the same arg order as the parent
                        // Only accept this candidate if its concrete args equal the parent's args
                        return *sub_args == parent_args;
                    }
                }
                true
            }
            (Predicate::Custom(exp_cpr), Stmt::Custom(sub_cpr, _)) => {
                // Direct custom predicate reference; match exactly
                *sub_cpr == *exp_cpr
            }
            (Predicate::Native(NP::SignedBy), Stmt::SignedBy(_, _)) => true,
            (Predicate::Native(NP::Equal), Stmt::Equal(_, _)) => true,
            (Predicate::Native(NP::Lt), Stmt::Lt(_, _)) => true,
            (Predicate::Native(NP::LtEq), Stmt::LtEq(_, _)) => true,
            (Predicate::Native(NP::SumOf), Stmt::SumOf(_, _, _)) => true,
            (Predicate::Native(NP::Contains), Stmt::Contains(_, _, _)) => true,
            _ => false,
        });
        if let Some(sref) = matched {
            let s = sref.clone();
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
    use pod2::middleware::{Statement as St, ValueRef as VR};
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
        VR::Literal(v) => format!("{}", v),
        VR::Key(ak) => format!("{}[\"{}\"]", ak.root, ak.key.name()),
    }
}

// (unit tests moved to integration tests)
