use pod2::middleware::{Hash, Key, Statement, StatementArg, StatementTmplArg, Value};

use crate::{
    edb::EdbView,
    prop::{wildcards_in_args, Choice, PropagatorResult},
    types::{ConstraintStore, OpTag, PodRef},
    util::{contains_stmt, tag_from_source},
};

/// Helper: classify an argument into a numeric int if possible, with premises when using AKs.
#[derive(Debug)]
pub enum NumArg {
    Ground {
        i: i64,
        premises: Vec<(Statement, OpTag)>,
    },
    AkVar {
        wc_index: usize,
        key: Key,
    },
    Wait(usize),
    TypeError,
    NoFact,
}

fn int_from_value(v: &Value) -> Result<i64, ()> {
    i64::try_from(v.typed()).map_err(|_| ())
}

pub fn classify_num(arg: &StatementTmplArg, store: &ConstraintStore, edb: &dyn EdbView) -> NumArg {
    match arg {
        StatementTmplArg::Literal(v) => match int_from_value(v) {
            Ok(i) => NumArg::Ground {
                i,
                premises: vec![],
            },
            Err(_) => NumArg::TypeError,
        },
        StatementTmplArg::Wildcard(w) => match store.bindings.get(&w.index) {
            Some(v) => match int_from_value(v) {
                Ok(i) => NumArg::Ground {
                    i,
                    premises: vec![],
                },
                Err(_) => NumArg::TypeError,
            },
            None => NumArg::Wait(w.index),
        },
        StatementTmplArg::AnchoredKey(w, key) => match store.bindings.get(&w.index) {
            Some(bound_root_val) => {
                let root: Hash = Hash::from(bound_root_val.raw());
                if let Some(val) = edb.contains_value(&root, key) {
                    if let Ok(i) = int_from_value(&val) {
                        let src = match edb.contains_source(&root, key, &val) {
                            Some(s) => s,
                            None => return NumArg::NoFact,
                        };
                        let tag = tag_from_source(key, &val, src);
                        let c = contains_stmt(root, key, val.clone());
                        NumArg::Ground {
                            i,
                            premises: vec![(c, tag)],
                        }
                    } else {
                        NumArg::TypeError
                    }
                } else {
                    NumArg::NoFact
                }
            }
            None => NumArg::Wait(w.index),
        },
        _ => NumArg::TypeError,
    }
}

pub fn arg_to_selector<'a>(
    arg: &'a StatementTmplArg,
    store: &'a ConstraintStore,
    tmp_val: &'a mut Option<Value>,
    tmp_root: &'a mut Option<Hash>,
) -> crate::edb::ArgSel<'a> {
    match arg {
        StatementTmplArg::Literal(v) => crate::edb::ArgSel::Literal(v),
        StatementTmplArg::Wildcard(w) => {
            if let Some(v) = store.bindings.get(&w.index) {
                *tmp_val = Some(v.clone());
                crate::edb::ArgSel::Literal(tmp_val.as_ref().unwrap())
            } else {
                crate::edb::ArgSel::Val
            }
        }
        StatementTmplArg::AnchoredKey(w, key) => {
            if let Some(root_val) = store.bindings.get(&w.index) {
                *tmp_root = Some(Hash::from(root_val.raw()));
                crate::edb::ArgSel::AkExact {
                    root: tmp_root.as_ref().unwrap(),
                    key,
                }
            } else {
                crate::edb::ArgSel::AkByKey(key)
            }
        }
        StatementTmplArg::None => crate::edb::ArgSel::Val,
    }
}

pub fn create_bindings(
    template_args: &[StatementTmplArg],
    result_stmt: &Statement,
    store: &ConstraintStore,
) -> Vec<(usize, Value)> {
    let mut bindings = Vec::new();
    let result_args = result_stmt.args();

    for (i, template_arg) in template_args.iter().enumerate() {
        if i >= result_args.len() {
            continue;
        }

        let result_arg = &result_args[i];
        match template_arg {
            StatementTmplArg::Wildcard(w) => {
                if !store.bindings.contains_key(&w.index) {
                    if let StatementArg::Literal(v) = result_arg {
                        bindings.push((w.index, v.clone()));
                    }
                }
            }
            StatementTmplArg::AnchoredKey(w, _key) => {
                if !store.bindings.contains_key(&w.index) {
                    if let StatementArg::Key(ak) = result_arg {
                        bindings.push((w.index, Value::from(ak.root)));
                    }
                }
            }
            _ => {}
        }
    }
    bindings
}

pub fn handle_copy_results(
    results: Vec<(Statement, PodRef)>,
    template_args: &[StatementTmplArg],
    store: &ConstraintStore,
) -> PropagatorResult {
    if results.is_empty() {
        let waits = wildcards_in_args(template_args)
            .into_iter()
            .filter(|i| !store.bindings.contains_key(i))
            .collect::<Vec<_>>();
        return if waits.is_empty() {
            PropagatorResult::Contradiction
        } else {
            PropagatorResult::Suspend { on: waits }
        };
    }

    let choices: Vec<Choice> = results
        .into_iter()
        .map(|(stmt, pod_ref)| {
            let bindings = create_bindings(template_args, &stmt, store);
            Choice {
                bindings,
                op_tag: OpTag::CopyStatement { source: pod_ref },
            }
        })
        .collect();

    if choices.is_empty() {
        PropagatorResult::Contradiction
    } else {
        PropagatorResult::Choices {
            alternatives: choices,
        }
    }
}
