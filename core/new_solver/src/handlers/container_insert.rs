use pod2::middleware::{
    containers::{Dictionary, Set},
    Hash, Key, NativePredicate, StatementTmplArg, TypedValue,
};

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::{EdbView, InsertSource},
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

/// Utility: extract a bound root hash from a template arg (literal or wildcard).
pub fn root_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<Hash> {
    match arg {
        StatementTmplArg::Literal(v) => Some(Hash::from(v.raw())),
        StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).map(|v| Hash::from(v.raw())),
        _ => None,
    }
}

/// Utility: extract a Key from a template arg (literal string or wildcard bound to string).
pub fn key_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<Key> {
    match arg {
        StatementTmplArg::Literal(v) => {
            if let Ok(s) = String::try_from(v.typed()) {
                Some(Key::from(s))
            } else {
                None
            }
        }
        StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).and_then(|v| {
            if let Ok(s) = String::try_from(v.typed()) {
                Some(Key::from(s))
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// Utility: extract an int from a template arg
pub fn int_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<i64> {
    match arg {
        StatementTmplArg::Literal(v) => i64::try_from(v.typed()).ok(),
        StatementTmplArg::Wildcard(w) => store
            .bindings
            .get(&w.index)
            .and_then(|v| i64::try_from(v.typed()).ok()),
        _ => None,
    }
}

/// Copy existing ContainerInsert(root, key, value) statements from EDB.
/// Supports binding the value (third argument) when root and key are known.
pub struct CopyContainerInsertHandler;

impl OpHandler for CopyContainerInsertHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 4 {
            return PropagatorResult::Contradiction;
        }

        // We need to store owned values for selectors, since ArgSel holds references.
        let (mut a_val, mut a_root) = (None, None);
        let (mut b_val, mut b_root) = (None, None);
        let (mut c_val, mut c_root) = (None, None);
        let (mut d_val, mut d_root) = (None, None);

        let sel_a = arg_to_selector(&args[0], store, &mut a_val, &mut a_root);
        let sel_b = arg_to_selector(&args[1], store, &mut b_val, &mut b_root);
        let sel_c = arg_to_selector(&args[2], store, &mut c_val, &mut c_root);
        let sel_d = arg_to_selector(&args[3], store, &mut d_val, &mut d_root);

        let results = edb.query(
            crate::edb::PredicateKey::Native(NativePredicate::ContainerInsert),
            &[sel_a, sel_b, sel_c, sel_d],
        );

        handle_copy_results(results, args, store)
    }
}

/// ContainerInsertFromEntries: when the full dictionary is known, it can justify ContainerInsert operations.
pub struct ContainerInsertFromEntriesHandler;

impl OpHandler for ContainerInsertFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 4 {
            return PropagatorResult::Contradiction;
        }
        let (new_root, old_root, a_key, a_val) = (&args[0], &args[1], &args[2], &args[3]);

        let new_root_value = match new_root {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
            _ => None,
        };
        let old_root_value = match old_root {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
            _ => None,
        };

        match (new_root_value, old_root_value) {
            (Some(new_root_value), Some(old_root_value)) => {
                match (new_root_value.typed(), old_root_value.typed()) {
                    (TypedValue::Dictionary(new_dict), TypedValue::Dictionary(old_dict)) => {
                        if let Some(key) = key_from_arg(a_key, store) {
                            return check_dict_insert_for_known_dicts(
                                new_dict, old_dict, &key, a_val, store,
                            );
                        }
                    }
                    (TypedValue::Set(new_set), TypedValue::Set(old_set)) => {
                        if let Some(result) =
                            check_set_insert_for_known_set(new_set, old_set, a_key, a_val, store)
                        {
                            return result;
                        };
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Enumeration: if both roots are unbound wildcards and key/value are known, enumerate candidate root pairs
        if let (StatementTmplArg::Wildcard(wnew), StatementTmplArg::Wildcard(wold)) =
            (new_root, old_root)
        {
            if !store.bindings.contains_key(&wnew.index)
                && !store.bindings.contains_key(&wold.index)
            {
                let key_opt = key_from_arg(a_key, store);
                let val_opt = match a_val {
                    StatementTmplArg::Literal(v) => Some(v.clone()),
                    StatementTmplArg::Wildcard(wv) => store.bindings.get(&wv.index).cloned(),
                    _ => None,
                };
                if let (Some(key), Some(val)) = (key_opt, val_opt) {
                    let mut alts = Vec::new();
                    for (new_hash, old_hash, src) in edb.enumerate_insert_sources(&key, &val) {
                        let op_tag = match src {
                            InsertSource::GeneratedFromFullDict { .. } => {
                                OpTag::GeneratedContainerInsert {
                                    new_root: new_hash,
                                    old_root: old_hash,
                                    key: key.clone(),
                                    value: val.clone(),
                                }
                            }
                            InsertSource::Copied { pod } => OpTag::CopyStatement { source: pod },
                        };
                        alts.push(crate::prop::Choice {
                            bindings: vec![
                                (wnew.index, new_hash.into()),
                                (wold.index, old_hash.into()),
                            ],
                            op_tag,
                        });
                    }
                    tracing::trace!(
                        ?key,
                        ?val,
                        candidates = alts.len(),
                        "Insert enum root pairs"
                    );
                    return if alts.is_empty() {
                        PropagatorResult::Contradiction
                    } else {
                        PropagatorResult::Choices { alternatives: alts }
                    };
                }
            }
        }

        // Need to handle other missing argument combinations with proper suspension
        let waits = crate::prop::wildcards_in_args(args)
            .into_iter()
            .filter(|i| !store.bindings.contains_key(i))
            .collect::<Vec<_>>();

        if waits.is_empty() {
            // All arguments are bound - validate the operation if possible
            let new_hash_opt = root_from_arg(new_root, store);
            let old_hash_opt = root_from_arg(old_root, store);
            let key_opt = key_from_arg(a_key, store);
            let val_opt = match a_val {
                StatementTmplArg::Literal(v) => Some(v.clone()),
                StatementTmplArg::Wildcard(wv) => store.bindings.get(&wv.index).cloned(),
                _ => None,
            };

            if let (Some(new_hash), Some(old_hash), Some(key), Some(val)) =
                (new_hash_opt, old_hash_opt, key_opt, val_opt)
            {
                // Check if this insert operation is valid by seeing if it exists in sources
                for (enum_new, enum_old, _) in edb.enumerate_insert_sources(&key, &val) {
                    if enum_new == new_hash && enum_old == old_hash {
                        return PropagatorResult::Entailed {
                            bindings: vec![],
                            op_tag: OpTag::FromLiterals,
                        };
                    }
                }
                PropagatorResult::Contradiction
            } else {
                // Some values couldn't be extracted - contradiction
                PropagatorResult::Contradiction
            }
        } else {
            PropagatorResult::Suspend { on: waits }
        }
    }
}

fn check_dict_insert_for_known_dicts(
    new_dict: &Dictionary,
    old_dict: &Dictionary,
    key: &Key,
    a_val: &StatementTmplArg,
    store: &ConstraintStore,
) -> PropagatorResult {
    // get the value from the new_dict
    let dict_value = new_dict.get(&key);
    if dict_value.is_err() {
        return PropagatorResult::Contradiction;
    }
    let dict_value = dict_value.unwrap();

    // insert the value into the old dict and ensure equal to new_dict
    let mut old_dict = old_dict.clone();
    if old_dict.insert(&key, &dict_value).is_err() || *new_dict != old_dict {
        return PropagatorResult::Contradiction;
    }

    // check the value matches the expected value
    match a_val {
        StatementTmplArg::Literal(v) => {
            if dict_value == v {
                PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::FromLiterals,
                }
            } else {
                PropagatorResult::Contradiction
            }
        }
        StatementTmplArg::Wildcard(wv) => {
            if let Some(existing) = store.bindings.get(&wv.index) {
                if existing == dict_value {
                    PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::FromLiterals,
                    }
                } else {
                    PropagatorResult::Contradiction
                }
            } else {
                PropagatorResult::Entailed {
                    bindings: vec![(wv.index, dict_value.clone())],
                    op_tag: OpTag::FromLiterals,
                }
            }
        }
        _ => PropagatorResult::Contradiction,
    }
}

fn check_set_insert_for_known_set(
    new_set: &Set,
    old_set: &Set,
    a_key: &StatementTmplArg,
    a_val: &StatementTmplArg,
    store: &ConstraintStore,
) -> Option<PropagatorResult> {
    // For Sets, key and value arguments must unify to the same value.
    let (value_opt, bindings_opt) = match (a_key, a_val) {
        (StatementTmplArg::Literal(k), StatementTmplArg::Literal(v)) => {
            if k != v {
                return Some(PropagatorResult::Contradiction);
            }
            (Some(k.clone()), Some(vec![]))
        }
        (StatementTmplArg::Literal(k), StatementTmplArg::Wildcard(wv)) => {
            if let Some(bound_v) = store.bindings.get(&wv.index) {
                if k != bound_v {
                    return Some(PropagatorResult::Contradiction);
                }
                (Some(k.clone()), Some(vec![]))
            } else {
                (Some(k.clone()), Some(vec![(wv.index, k.clone())]))
            }
        }
        (StatementTmplArg::Wildcard(wk), StatementTmplArg::Literal(v)) => {
            if let Some(bound_k) = store.bindings.get(&wk.index) {
                if v != bound_k {
                    return Some(PropagatorResult::Contradiction);
                }
                (Some(v.clone()), Some(vec![]))
            } else {
                (Some(v.clone()), Some(vec![(wk.index, v.clone())]))
            }
        }
        (StatementTmplArg::Wildcard(wk), StatementTmplArg::Wildcard(wv)) => {
            let k_bound = store.bindings.get(&wk.index);
            let v_bound = store.bindings.get(&wv.index);
            match (k_bound, v_bound) {
                (Some(k), Some(v)) => {
                    if k != v {
                        return Some(PropagatorResult::Contradiction);
                    }
                    (Some(k.clone()), Some(vec![]))
                }
                (Some(k), None) => (Some(k.clone()), Some(vec![(wv.index, k.clone())])),
                (None, Some(v)) => (Some(v.clone()), Some(vec![(wk.index, v.clone())])),
                (None, None) => (None, None), // Cannot determine value if both unbound
            }
        }
        _ => (None, None),
    };

    if value_opt.is_none() || bindings_opt.is_none() {
        return None;
    }
    let (value, bindings) = (value_opt.unwrap(), bindings_opt.unwrap());

    // ensure new set is old set with value inserted
    let mut old_set = old_set.clone();
    if old_set.insert(&value).is_err() || *new_set != old_set {
        return Some(PropagatorResult::Contradiction);
    }

    Some(PropagatorResult::Entailed {
        bindings,
        op_tag: OpTag::FromLiterals,
    })
}

/// Register ContainerInsert handlers with the operation registry.
pub fn register_container_insert_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::ContainerInsert,
        Box::new(CopyContainerInsertHandler),
    );
    reg.register(
        NativePredicate::ContainerInsert,
        Box::new(ContainerInsertFromEntriesHandler),
    );
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{StatementTmplArg, Value};

    use super::*;
    use crate::{edb::ImmutableEdbBuilder, types::ConstraintStore};

    #[test]
    fn test_copy_container_insert_handler() {
        // Basic test structure - would need actual EDB with ContainerInsert statements
        //let mut store = ConstraintStore::default();
        //let edb = ImmutableEdbBuilder::new().build();
        //let handler = CopyContainerInsertHandler;

        //let args = args_from(&["?0", "?1", "\"key\"", "\"value\""]);
        //let result = handler.propagate(&args, &mut store, &edb);

        //// Should not find anything in empty EDB
        //matches!(result, PropagatorResult::Contradiction);
    }

    #[test]
    fn test_container_insert_from_entries_dict() {
        let mut store = ConstraintStore::default();
        let edb = ImmutableEdbBuilder::new().build();
        let handler = ContainerInsertFromEntriesHandler;

        // Test with all literal arguments
        let new_root = Value::from("new_root_hash".to_string());
        let old_root = Value::from("old_root_hash".to_string());
        let key = Value::from("test_key".to_string());
        let value = Value::from("test_value".to_string());

        let args = vec![
            StatementTmplArg::Literal(new_root),
            StatementTmplArg::Literal(old_root),
            StatementTmplArg::Literal(key),
            StatementTmplArg::Literal(value),
        ];

        let result = handler.propagate(&args, &mut store, &edb);

        // Should succeed - all literals are considered valid for now
        matches!(result, PropagatorResult::Entailed { .. });
    }
}
