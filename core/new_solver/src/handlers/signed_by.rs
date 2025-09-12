use pod2::middleware::{NativePredicate, StatementTmplArg, Value};
use tracing::trace;

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

/// Copy SignedBy: copy existing SignedBy(Value, PublicKey) rows.
pub struct CopySignedByHandler;

impl OpHandler for CopySignedByHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        trace!("SignedBy(copy): args={:?}", args);

        // We need to store owned values for selectors, since ArgSel holds references.
        let (mut l_val, mut l_root) = (None, None);
        let (mut r_val, mut r_root) = (None, None);

        let lhs = arg_to_selector(&args[0], store, &mut l_val, &mut l_root);
        let rhs = arg_to_selector(&args[1], store, &mut r_val, &mut r_root);

        let results = edb.query(
            crate::edb::PredicateKey::Native(NativePredicate::SignedBy),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

/// SignedBy generator: when left is a SignedDict (literal or bound via root), verify and emit.
pub struct SignedByHandler;

impl OpHandler for SignedByHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        // Left: any Value whose hash is the signed root. Prefer a concrete SignedDict literal
        // or resolve from EDB if left is a root.
        // Right: public key literal or wildcard.
        // Cases:
        // 1) Left is Literal(SignedDict): verify; bind right wildcard if needed; entail.
        // 2) Left is Wildcard bound to a literal SignedDict: same as 1.
        // 3) Left is Literal(root hash) or wildcard bound to a root: look up EDB.signed_dict(root) and verify.
        // Otherwise: suspend on wildcards that could become SignedDict/root.

        // Left as root (literal or bound)
        let maybe_root = match &args[0] {
            StatementTmplArg::Literal(v) => Some(pod2::middleware::Hash::from(v.raw())),
            StatementTmplArg::Wildcard(w) => store
                .bindings
                .get(&w.index)
                .map(|v| pod2::middleware::Hash::from(v.raw())),
            _ => None,
        };

        if let Some(root) = maybe_root {
            let sd = match edb.signed_dict(&root) {
                Some(sd) => sd,
                None => return PropagatorResult::Contradiction,
            };
            let pk_from_signature = sd.public_key;
            let pk_val_from_signature = Value::from(pk_from_signature);

            // Resolve the public key provided in the second argument
            match &args[1] {
                StatementTmplArg::Literal(v) => {
                    if v.raw() != pk_val_from_signature.raw() {
                        return PropagatorResult::Contradiction;
                    }
                    return PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::FromLiterals,
                    };
                }
                StatementTmplArg::Wildcard(w) => {
                    if let Some(bound_pk) = store.bindings.get(&w.index) {
                        if bound_pk.raw() != pk_val_from_signature.raw() {
                            return PropagatorResult::Contradiction;
                        }
                        return PropagatorResult::Entailed {
                            bindings: vec![],
                            op_tag: OpTag::FromLiterals,
                        };
                    } else {
                        return PropagatorResult::Entailed {
                            bindings: vec![(w.index, pk_val_from_signature)],
                            op_tag: OpTag::FromLiterals,
                        };
                    }
                }
                StatementTmplArg::AnchoredKey(w_ak, k) => {
                    if let Some(ak_root_val) = store.bindings.get(&w_ak.index) {
                        let ak_root_hash = pod2::middleware::Hash::from(ak_root_val.raw());
                        if let Some(pk_val_from_ak) = edb.contains_value(&ak_root_hash, k) {
                            if pk_val_from_ak.raw() != pk_val_from_signature.raw() {
                                return PropagatorResult::Contradiction;
                            }

                            if let Some(source) =
                                edb.contains_source(&ak_root_hash, k, &pk_val_from_ak)
                            {
                                let premise_stmt = pod2::middleware::Statement::Contains(
                                    ak_root_val.clone().into(),
                                    k.name().into(),
                                    pk_val_from_ak.clone().into(),
                                );
                                let premise_tag = match source {
                                    crate::edb::ContainsSource::Copied { pod } => {
                                        OpTag::CopyStatement { source: pod }
                                    }
                                    crate::edb::ContainsSource::GeneratedFromFullDict {
                                        root,
                                        ..
                                    } => OpTag::GeneratedContains {
                                        root,
                                        key: k.clone(),
                                        value: pk_val_from_ak,
                                    },
                                };

                                return PropagatorResult::Entailed {
                                    bindings: vec![],
                                    op_tag: OpTag::Derived {
                                        premises: vec![(premise_stmt, premise_tag)],
                                    },
                                };
                            } else {
                                return PropagatorResult::Contradiction;
                            }
                        } else {
                            return PropagatorResult::Contradiction;
                        }
                    } else {
                        return PropagatorResult::Suspend {
                            on: vec![w_ak.index],
                        };
                    }
                }
                _ => return PropagatorResult::Contradiction,
            }
        }

        // Under-constrained: suspend on unbound wildcards
        let waits = crate::prop::wildcards_in_args(args)
            .into_iter()
            .filter(|i| !store.bindings.contains_key(i))
            .collect::<Vec<_>>();
        if waits.is_empty() {
            PropagatorResult::Contradiction
        } else {
            PropagatorResult::Suspend { on: waits }
        }
    }
}

pub fn register_signed_by_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::SignedBy, Box::new(CopySignedByHandler));
    reg.register(NativePredicate::SignedBy, Box::new(SignedByHandler));
}

#[cfg(test)]
mod tests {
    use pod2::{
        backends::plonky2::signer::Signer,
        frontend::SignedDictBuilder,
        middleware::{Params, SecretKey},
    };

    use super::*;
    use crate::{edb::ImmutableEdbBuilder, test_helpers::args_from, types::ConstraintStore};

    #[test]
    fn signed_by_verify_success() {
        let sk = SecretKey::new_rand();
        let pk = sk.public_key();
        let params = Params::default();

        let mut builder = SignedDictBuilder::new(&params);
        builder.insert("a", 1i64);
        let signer = Signer(sk);
        let sd = builder.sign(&signer).unwrap();
        let root = sd.dict.commitment();

        let edb = ImmutableEdbBuilder::new().add_signed_dict(sd).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));
        store.bindings.insert(1, Value::from(pk));

        let handler = SignedByHandler;
        let args = args_from("REQUEST(SignedBy(R, PK))");

        let res = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(res, PropagatorResult::Entailed { .. }));
    }

    #[test]
    fn signed_by_verify_failure_wrong_key() {
        let sk1 = SecretKey::new_rand();
        let params = Params::default();
        let mut builder = SignedDictBuilder::new(&params);
        builder.insert("a", 1i64);
        let signer = Signer(sk1);
        let sd = builder.sign(&signer).unwrap();
        let root = sd.dict.commitment();

        let sk2 = SecretKey::new_rand();
        let pk2 = sk2.public_key();

        let edb = ImmutableEdbBuilder::new().add_signed_dict(sd).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));
        store.bindings.insert(1, Value::from(pk2));

        let handler = SignedByHandler;
        let args = args_from("REQUEST(SignedBy(R, PK))");

        let res = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(res, PropagatorResult::Contradiction));
    }

    #[test]
    fn signed_by_generate_pk() {
        let sk = SecretKey::new_rand();
        let pk = sk.public_key();
        let params = Params::default();

        let mut builder = SignedDictBuilder::new(&params);
        builder.insert("a", 1i64);
        let signer = Signer(sk);
        let sd = builder.sign(&signer).unwrap();
        let root = sd.dict.commitment();

        let edb = ImmutableEdbBuilder::new().add_signed_dict(sd).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));

        let handler = SignedByHandler;
        let args = args_from("REQUEST(SignedBy(R, PK))");

        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, .. } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, 1); // PK index
                assert_eq!(bindings[0].1, Value::from(pk));
            }
            other => panic!("Unexpected result: {other:?}"),
        }
    }

    #[test]
    fn signed_by_suspend_unbound_root() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = SignedByHandler;
        let args = args_from("REQUEST(SignedBy(R, PK))");

        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => {
                assert_eq!(on.len(), 2);
                assert!(on.contains(&0)); // R
                assert!(on.contains(&1)); // PK
            }
            other => panic!("Unexpected result: {other:?}"),
        }
    }
}
