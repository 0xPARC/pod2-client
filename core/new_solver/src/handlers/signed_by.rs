use pod2::middleware::{NativePredicate, StatementTmplArg, Value};
use tracing::trace;

use crate::{
    edb::{ArgSel, BinaryPred, EdbView},
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
    util::binary_view,
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
        // Fast path: enumerate roots by known PK when left is an unbound wildcard
        let pk_opt: Option<Value> = match &args[1] {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
            _ => None,
        };
        if let (StatementTmplArg::Wildcard(wl), Some(pk)) = (&args[0], pk_opt.clone()) {
            if !store.bindings.contains_key(&wl.index) {
                let mut alts: Vec<crate::prop::Choice> = Vec::new();
                let all = edb.enumerate_signed_dicts();
                let mut matched = 0usize;
                for sd in all.into_iter() {
                    if Value::from(sd.public_key).raw() == pk.raw() {
                        let root = sd.dict.commitment();
                        let mut binds = vec![(wl.index, Value::from(root))];
                        if let StatementTmplArg::Wildcard(wr) = &args[1] {
                            if !store.bindings.contains_key(&wr.index) {
                                binds.push((wr.index, pk.clone()));
                            }
                        }
                        alts.push(crate::prop::Choice {
                            bindings: binds,
                            op_tag: OpTag::FromLiterals,
                        });
                        matched += 1;
                    }
                }
                trace!(?pk, matched, "SignedBy enum by PK");
                return if alts.is_empty() {
                    PropagatorResult::Contradiction
                } else {
                    PropagatorResult::Choices { alternatives: alts }
                };
            }
        }
        // Build selectors independently per side
        let sel_l = match &args[0] {
            StatementTmplArg::Literal(v) => ArgSel::Literal(v),
            StatementTmplArg::Wildcard(w) => match store.bindings.get(&w.index) {
                Some(v) => ArgSel::Literal(v),
                None => ArgSel::Val,
            },
            // AK not meaningful for SignedBy; leave unconstrained
            StatementTmplArg::AnchoredKey(_, _) => ArgSel::Val,
            StatementTmplArg::None => ArgSel::Val,
        };
        let sel_r = match &args[1] {
            StatementTmplArg::Literal(v) => ArgSel::Literal(v),
            StatementTmplArg::Wildcard(w) => match store.bindings.get(&w.index) {
                Some(v) => ArgSel::Literal(v),
                None => ArgSel::Val,
            },
            // AK not meaningful here; leave unconstrained
            StatementTmplArg::AnchoredKey(_, _) | StatementTmplArg::None => ArgSel::Val,
        };

        let mut choices: Vec<crate::prop::Choice> = Vec::new();
        for row in binary_view(edb, BinaryPred::SignedBy, sel_l, sel_r).into_iter() {
            let mut binds: Vec<(usize, Value)> = Vec::new();
            // Left side bindings (wildcard literal)
            if let StatementTmplArg::Wildcard(wl) = &args[0] {
                if !store.bindings.contains_key(&wl.index) {
                    if let Some(v) = row.left.as_literal() {
                        binds.push((wl.index, v.clone()));
                    }
                }
            }
            // Right side bindings (wildcard public key as Value)
            if let StatementTmplArg::Wildcard(wr) = &args[1] {
                if !store.bindings.contains_key(&wr.index) {
                    if let Some(v) = row.right.as_literal() {
                        binds.push((wr.index, v.clone()));
                    }
                }
            }
            choices.push(crate::prop::Choice {
                bindings: binds,
                op_tag: OpTag::CopyStatement { source: row.src },
            });
        }
        trace!(choices = choices.len(), "SignedBy(copy) normalized choices");
        if choices.is_empty() {
            // Suspend on referenced unbound wildcards
            let waits = crate::prop::wildcards_in_args(args)
                .into_iter()
                .filter(|i| !store.bindings.contains_key(i))
                .collect::<Vec<_>>();
            if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            }
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
