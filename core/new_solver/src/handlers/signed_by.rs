use pod2::middleware::{NativePredicate, StatementTmplArg, Value};
use tracing::trace;

use super::util::{arg_to_selector, create_bindings};
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

        if results.is_empty() {
            let waits = crate::prop::wildcards_in_args(args)
                .into_iter()
                .filter(|i| !store.bindings.contains_key(i))
                .collect::<Vec<_>>();
            return if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            };
        }

        let choices: Vec<crate::prop::Choice> = results
            .into_iter()
            .map(|(stmt, pod_ref)| {
                let bindings = create_bindings(args, &stmt, store);
                crate::prop::Choice {
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
            let has = edb.signed_dict(&root).is_some();
            trace!(?root, found = has, "SignedBy lookup by root");
            if let Some(sd) = edb.signed_dict(&root) {
                let pk_val = Value::from(sd.public_key);
                // If right is a literal pk, require equality to embedded pk
                if let StatementTmplArg::Literal(v) = &args[1] {
                    if v.raw() != pk_val.raw() {
                        return PropagatorResult::Contradiction;
                    }
                }
                let mut binds: Vec<(usize, Value)> = Vec::new();
                if let StatementTmplArg::Wildcard(wr) = &args[1] {
                    if !store.bindings.contains_key(&wr.index) {
                        binds.push((wr.index, pk_val));
                    }
                }
                return PropagatorResult::Entailed {
                    bindings: binds,
                    op_tag: OpTag::FromLiterals,
                };
            } else {
                return PropagatorResult::Contradiction;
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
