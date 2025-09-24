use pod2::middleware::{NativePredicate, PublicKey, SecretKey, StatementTmplArg, Value};
use tracing::trace;

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

/// Copy PublicKeyOf: copy existing PublicKeyOf(Value, PublicKey) rows.
pub struct CopyPublicKeyOfHandler;

impl OpHandler for CopyPublicKeyOfHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        trace!("PublicKeyOf(copy): args={:?}", args);

        // We need to store owned values for selectors, since ArgSel holds references.
        let (mut l_val, mut l_root) = (None, None);
        let (mut r_val, mut r_root) = (None, None);

        let lhs = arg_to_selector(&args[0], store, &mut l_val, &mut l_root);
        let rhs = arg_to_selector(&args[1], store, &mut r_val, &mut r_root);

        let results = edb.query(
            crate::edb::PredicateKey::Native(NativePredicate::PublicKeyOf),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

/// SignedBy generator: when left is a SignedDict (literal or bound via root), verify and emit.
pub struct PublicKeyOfHandler;

impl OpHandler for PublicKeyOfHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }

        let a_public_key = &args[0];
        let a_secret_key = &args[1];

        // Extract secret key if available
        let sk_opt: Option<SecretKey> = match a_secret_key {
            StatementTmplArg::Literal(v) => v.typed().try_into().ok(),
            StatementTmplArg::Wildcard(w) => store
                .bindings
                .get(&w.index)
                .and_then(|v| v.typed().try_into().ok()),
            _ => None,
        };

        // Extract public key if available
        let pk_opt: Option<PublicKey> = match a_public_key {
            StatementTmplArg::Literal(v) => v.typed().try_into().ok(),
            StatementTmplArg::Wildcard(w) => store
                .bindings
                .get(&w.index)
                .and_then(|v| v.typed().try_into().ok()),
            _ => None,
        };

        match (sk_opt, pk_opt) {
            // Case 1: Secret key is known. We can derive the public key.
            (Some(sk), _) => {
                let derived_pk = sk.public_key();
                let derived_pk_val = Value::from(derived_pk);

                match a_public_key {
                    // If PK arg is a wildcard, bind it if unbound, or check if it matches.
                    StatementTmplArg::Wildcard(w) => {
                        if let Some(bound_pk) = store.bindings.get(&w.index) {
                            if bound_pk == &derived_pk_val {
                                PropagatorResult::Entailed {
                                    bindings: vec![],
                                    op_tag: OpTag::FromLiterals,
                                }
                            } else {
                                PropagatorResult::Contradiction
                            }
                        } else {
                            PropagatorResult::Entailed {
                                bindings: vec![(w.index, derived_pk_val)],
                                op_tag: OpTag::FromLiterals,
                            }
                        }
                    }
                    // If PK arg is a literal, check for equality.
                    StatementTmplArg::Literal(v) => {
                        if v == &derived_pk_val {
                            PropagatorResult::Entailed {
                                bindings: vec![],
                                op_tag: OpTag::FromLiterals,
                            }
                        } else {
                            PropagatorResult::Contradiction
                        }
                    }
                    _ => PropagatorResult::Contradiction,
                }
            }

            // Case 2: Secret key is unknown, but public key is known.
            (None, Some(pk)) => {
                if let Some(sk_from_edb) = edb.get_secret_key(&pk) {
                    let sk_val = Value::from(sk_from_edb.clone());
                    match a_secret_key {
                        // If SK arg is a wildcard, bind it. Since sk_opt is None, we know it's unbound.
                        StatementTmplArg::Wildcard(w) => PropagatorResult::Entailed {
                            bindings: vec![(w.index, sk_val)],
                            op_tag: OpTag::FromLiterals,
                        },
                        // This case should be impossible, but we handle it defensively.
                        StatementTmplArg::Literal(v) => {
                            if v == &sk_val {
                                PropagatorResult::Entailed {
                                    bindings: vec![],
                                    op_tag: OpTag::FromLiterals,
                                }
                            } else {
                                PropagatorResult::Contradiction
                            }
                        }
                        _ => PropagatorResult::Contradiction,
                    }
                } else {
                    // We have a public key, but no corresponding secret key in the EDB.
                    PropagatorResult::Contradiction
                }
            }

            // Case 3: Neither secret key nor public key is known.
            (None, None) => {
                // Enumeration: when both are unbound wildcards, enumerate all keypairs from the EDB
                if let (StatementTmplArg::Wildcard(wsk), StatementTmplArg::Wildcard(wpk)) =
                    (a_secret_key, a_public_key)
                {
                    if !store.bindings.contains_key(&wsk.index)
                        && !store.bindings.contains_key(&wpk.index)
                    {
                        let mut alts = Vec::new();
                        for (pk_val, sk_val) in edb.enumerate_keypairs() {
                            // Extract the actual keypair values for the OpTag
                            if let (Ok(sk), Ok(pk)) = (
                                pod2::middleware::SecretKey::try_from(sk_val.typed()),
                                pod2::middleware::PublicKey::try_from(pk_val.typed()),
                            ) {
                                alts.push(crate::prop::Choice {
                                    bindings: vec![(wsk.index, sk_val), (wpk.index, pk_val)],
                                    op_tag: OpTag::GeneratedPublicKeyOf {
                                        secret_key: sk,
                                        public_key: pk,
                                    },
                                });
                            }
                        }
                        tracing::trace!(candidates = alts.len(), "PublicKeyOf enum all keypairs");
                        return if alts.is_empty() {
                            PropagatorResult::Contradiction
                        } else {
                            PropagatorResult::Choices { alternatives: alts }
                        };
                    }
                }

                let waits = crate::prop::wildcards_in_args(args)
                    .into_iter()
                    .filter(|i| !store.bindings.contains_key(i))
                    .collect::<Vec<_>>();
                if waits.is_empty() {
                    // Both args are literals but we couldn't parse them.
                    PropagatorResult::Contradiction
                } else {
                    PropagatorResult::Suspend { on: waits }
                }
            }
        }
    }
}

pub fn register_publickeyof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::PublicKeyOf,
        Box::new(CopyPublicKeyOfHandler),
    );
    reg.register(NativePredicate::PublicKeyOf, Box::new(PublicKeyOfHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::SecretKey;

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::args_from,
        types::{ConstraintStore, OpTag},
    };

    #[test]
    fn publickeyof_from_entries_generates_pk_from_sk() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let sk = SecretKey::new_rand();
        let pk_val = Value::from(sk.public_key());
        store.bindings.insert(1, Value::from(sk));

        let handler = PublicKeyOfHandler;
        let args = args_from("REQUEST(PublicKeyOf(PK, SK))");
        let res = handler.propagate(&args, &mut store, &edb);

        match res {
            PropagatorResult::Entailed { bindings, op_tag } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, 0); // PK index
                assert_eq!(bindings[0].1, pk_val);
                assert!(matches!(op_tag, OpTag::FromLiterals));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn publickeyof_from_entries_sk_from_pk() {
        let sk = SecretKey::new_rand();
        let pk = sk.public_key();
        let pk_val = Value::from(pk);
        let sk_val = Value::from(sk.clone());

        let edb = ImmutableEdbBuilder::new().add_keypair(pk, sk).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, pk_val.clone());

        let handler = PublicKeyOfHandler;
        let args = args_from("REQUEST(PublicKeyOf(PK, SK))");
        let res = handler.propagate(&args, &mut store, &edb);

        match res {
            PropagatorResult::Entailed { bindings, op_tag } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, 1); // SK index
                assert_eq!(bindings[0].1, sk_val);
                assert!(matches!(op_tag, OpTag::FromLiterals));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
