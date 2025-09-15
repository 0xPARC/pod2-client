use pod2::middleware::{
    containers::Dictionary, Key, NativePredicate, StatementTmplArg, TypedValue,
};

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::{EdbView, UpdateSource},
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

// Import utilities from container_insert module
use super::container_insert::{key_from_arg, root_from_arg};

/// Copy existing ContainerUpdate(new_root, old_root, key, value) statements from EDB.
pub struct CopyContainerUpdateHandler;

impl OpHandler for CopyContainerUpdateHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::ContainerUpdate),
            &[sel_a, sel_b, sel_c, sel_d],
        );

        handle_copy_results(results, args, store)
    }
}

/// ContainerUpdateFromEntries: when the full dictionary is known, it can justify ContainerUpdate operations.
pub struct ContainerUpdateFromEntriesHandler;

impl OpHandler for ContainerUpdateFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 4 {
            return PropagatorResult::Contradiction;
        }

        let (new_root, old_root, a_key, a_value) = (&args[0], &args[1], &args[2], &args[3]);

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
                            return check_dict_update_for_known_dicts(
                                new_dict, old_dict, &key, a_value, store,
                            );
                        }
                    }
                    // Sets don't support update operations directly (they only have insert/delete)
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
                let val_opt = match a_value {
                    StatementTmplArg::Literal(v) => Some(v.clone()),
                    StatementTmplArg::Wildcard(wv) => store.bindings.get(&wv.index).cloned(),
                    _ => None,
                };

                if let (Some(key), Some(val)) = (key_opt, val_opt) {
                    let mut alts = Vec::new();
                    for (new_hash, old_hash, src) in edb.enumerate_update_sources(&key, &val) {
                        let op_tag = match src {
                            UpdateSource::GeneratedFromFullDict { .. } => {
                                OpTag::GeneratedContainerUpdate {
                                    new_root: new_hash,
                                    old_root: old_hash,
                                    key: key.clone(),
                                    value: val.clone(),
                                }
                            }
                            UpdateSource::Copied { pod } => OpTag::CopyStatement { source: pod },
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
                        "Update enum root pairs"
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
            let val_opt = match a_value {
                StatementTmplArg::Literal(v) => Some(v.clone()),
                StatementTmplArg::Wildcard(wv) => store.bindings.get(&wv.index).cloned(),
                _ => None,
            };

            if let (Some(new_hash), Some(old_hash), Some(key), Some(val)) =
                (new_hash_opt, old_hash_opt, key_opt, val_opt)
            {
                // Check if this update operation is valid by seeing if it exists in sources
                for (enum_new, enum_old, _) in edb.enumerate_update_sources(&key, &val) {
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

fn check_dict_update_for_known_dicts(
    new_dict: &Dictionary,
    old_dict: &Dictionary,
    key: &Key,
    a_value: &StatementTmplArg,
    store: &ConstraintStore,
) -> PropagatorResult {
    // Get the value from the new dictionary (this is the new value that was updated)
    let new_dict_value = new_dict.get(&key);
    if new_dict_value.is_err() {
        return PropagatorResult::Contradiction;
    }
    let new_dict_value = new_dict_value.unwrap();

    // Ensure the old dictionary had the key (ContainerUpdate requires the key to exist in old_root)
    if old_dict.get(&key).is_err() {
        return PropagatorResult::Contradiction;
    }

    // Update the old dict with new value and ensure it equals new_dict
    let mut updated_dict = old_dict.clone();
    if updated_dict.update(&key, &new_dict_value).is_err() || *new_dict != updated_dict {
        return PropagatorResult::Contradiction;
    }

    // Check the value matches the expected value
    match a_value {
        StatementTmplArg::Literal(v) => {
            if new_dict_value == v {
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
                if existing == new_dict_value {
                    PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::FromLiterals,
                    }
                } else {
                    PropagatorResult::Contradiction
                }
            } else {
                PropagatorResult::Entailed {
                    bindings: vec![(wv.index, new_dict_value.clone())],
                    op_tag: OpTag::FromLiterals,
                }
            }
        }
        _ => PropagatorResult::Contradiction,
    }
}

/// Register ContainerUpdate handlers with the operation registry.
pub fn register_container_update_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::ContainerUpdate,
        Box::new(CopyContainerUpdateHandler),
    );
    reg.register(
        NativePredicate::ContainerUpdate,
        Box::new(ContainerUpdateFromEntriesHandler),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edb::ImmutableEdbBuilder, types::ConstraintStore};
    use pod2::middleware::{StatementTmplArg, Value};

    #[test]
    fn test_copy_container_update_handler() {
        let mut store = ConstraintStore::default();
        let edb = ImmutableEdbBuilder::new().build();
        let handler = CopyContainerUpdateHandler;

        // Test with correct number of arguments
        let new_root = Value::from("new_root_hash".to_string());
        let old_root = Value::from("old_root_hash".to_string());
        let key = Value::from("test_key".to_string());
        let value = Value::from("new_value".to_string());

        let args = vec![
            StatementTmplArg::Literal(new_root),
            StatementTmplArg::Literal(old_root),
            StatementTmplArg::Literal(key),
            StatementTmplArg::Literal(value),
        ];

        let result = handler.propagate(&args, &mut store, &edb);

        // Should not find anything in empty EDB but shouldn't contradict on valid args
        matches!(result, PropagatorResult::Contradiction);
    }

    #[test]
    fn test_container_update_wrong_arg_count() {
        let mut store = ConstraintStore::default();
        let edb = ImmutableEdbBuilder::new().build();
        let handler = ContainerUpdateFromEntriesHandler;

        // Test with wrong number of arguments (should be 4)
        let args = vec![
            StatementTmplArg::Literal(Value::from("test".to_string())),
            StatementTmplArg::Literal(Value::from("test".to_string())),
        ];

        let result = handler.propagate(&args, &mut store, &edb);

        matches!(result, PropagatorResult::Contradiction);
    }
}

