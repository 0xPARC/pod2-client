use std::{collections::HashMap, sync::Arc};

use pod2::middleware::{
    CustomPredicateBatch, CustomPredicateRef, Predicate, StatementTmpl, StatementTmplArg, Wildcard,
};

/// A simple conjunctive rule: head(args) :- AND(body...)
#[derive(Clone, Debug)]
pub struct CustomRule {
    pub pred: CustomPredicateRef,
    pub head: Vec<StatementTmplArg>,
    pub body: Vec<StatementTmpl>,
    /// Minimal native steps in the body (each native literal contributes at least 1).
    pub min_native_cost: usize,
    /// Number of custom subcalls in the body (each contributes at least 1 downstream step).
    pub min_subcall_count: usize,
}

#[derive(Default)]
pub struct RuleRegistry {
    rules: HashMap<CustomPredicateRef, Vec<CustomRule>>,
    /// Registration-time warnings (skipped/rewritten branches, recursion rejections, etc.).
    pub warnings: Vec<String>,
}

impl RuleRegistry {
    pub fn register(&mut self, rule: CustomRule) {
        self.rules.entry(rule.pred.clone()).or_default().push(rule);
    }

    pub fn get(&self, pred: &CustomPredicateRef) -> &[CustomRule] {
        self.rules.get(pred).map(|v| &v[..]).unwrap_or(&[])
    }

    pub fn push_warning(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
    pub fn clear_warnings(&mut self) {
        self.warnings.clear();
    }
}

/// Remap wildcards in a template arg according to `map`.
pub fn remap_arg(arg: &StatementTmplArg, map: &HashMap<usize, usize>) -> StatementTmplArg {
    match arg {
        StatementTmplArg::Wildcard(w) => StatementTmplArg::Wildcard(pod2::middleware::Wildcard {
            name: w.name.clone(),
            index: *map.get(&w.index).unwrap_or(&w.index),
        }),
        StatementTmplArg::AnchoredKey(w, key) => StatementTmplArg::AnchoredKey(
            pod2::middleware::Wildcard {
                name: w.name.clone(),
                index: *map.get(&w.index).unwrap_or(&w.index),
            },
            key.clone(),
        ),
        StatementTmplArg::Literal(v) => StatementTmplArg::Literal(v.clone()),
        StatementTmplArg::None => StatementTmplArg::None,
    }
}

pub fn remap_tmpl(t: &StatementTmpl, map: &HashMap<usize, usize>) -> StatementTmpl {
    StatementTmpl {
        pred: t.pred.clone(),
        args: t.args.iter().map(|a| remap_arg(a, map)).collect(),
    }
}

fn resolve_batchself(t: &StatementTmpl, batch: &Arc<CustomPredicateBatch>) -> StatementTmpl {
    match t.pred() {
        Predicate::BatchSelf(idx) => StatementTmpl {
            pred: Predicate::Custom(CustomPredicateRef::new(batch.clone(), *idx)),
            args: t.args.clone(),
        },
        _ => t.clone(),
    }
}

/// Convenience: register all predicates in a parsed `CustomPredicateBatch` as simple conjunctive rules.
/// Uses the predicate's `statements` as body and constructs the head from the first `args_len` wildcard names
/// with indices 0..args_len-1 (matching the batch's head convention).
pub fn register_rules_from_batch(reg: &mut RuleRegistry, batch: &Arc<CustomPredicateBatch>) {
    for (i, pred) in batch.predicates().iter().enumerate() {
        let args_len = pred.args_len();
        let head: Vec<StatementTmplArg> = pred
            .wildcard_names()
            .iter()
            .take(args_len)
            .enumerate()
            .map(|(idx, name)| StatementTmplArg::Wildcard(Wildcard::new(name.clone(), idx)))
            .collect();
        let cpr = CustomPredicateRef::new(batch.clone(), i);

        if pred.is_conjunction() {
            // Resolve BatchSelf references to CustomPredicateRef first
            let resolved: Vec<StatementTmpl> = pred
                .statements()
                .iter()
                .map(|t| resolve_batchself(t, batch))
                .collect();
            // Allow native and non-recursive custom subcalls; reject if any self-recursive reference appears
            let mut ok = true;
            for t in resolved.iter() {
                match t.pred() {
                    Predicate::Native(_) => {}
                    Predicate::Custom(other) => {
                        if other == &cpr {
                            log::warn!("Rejecting self-recursive AND body statement in {cpr:?}");
                            reg.push_warning(format!(
                                "Rejecting self-recursive AND statement in {cpr:?}"
                            ));
                            ok = false;
                            break;
                        }
                    }
                    Predicate::BatchSelf(_) | Predicate::Intro(_) => {
                        // Should not happen after resolution, but guard anyway
                        log::warn!(
                            "Skipping unsupported AND statement {:?} in {:?}",
                            t.pred(),
                            cpr
                        );
                        reg.push_warning(format!(
                            "Skipping unsupported AND statement {:?} in {:?}",
                            t.pred(),
                            cpr
                        ));
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                let mut min_native_cost = 0usize;
                let mut min_subcall_count = 0usize;
                for t in resolved.iter() {
                    match t.pred() {
                        Predicate::Native(_) => min_native_cost = min_native_cost.saturating_add(1),
                        Predicate::Custom(_) => {
                            min_subcall_count = min_subcall_count.saturating_add(1)
                        }
                        _ => {}
                    }
                }
                reg.register(CustomRule {
                    pred: cpr,
                    head,
                    body: resolved,
                    min_native_cost,
                    min_subcall_count,
                });
            }
        } else {
            // Disjunction: resolve BatchSelf, then create one rule per branch
            for st in pred
                .statements()
                .iter()
                .map(|t| resolve_batchself(t, batch))
            {
                match st.pred() {
                    Predicate::Native(_) => {
                        reg.register(CustomRule {
                            pred: cpr.clone(),
                            head: head.clone(),
                            body: vec![st.clone()],
                            min_native_cost: 1,
                            min_subcall_count: 0,
                        });
                    }
                    Predicate::Custom(other) => {
                        if other == &cpr {
                            log::warn!("Rejecting self-recursive OR branch in {cpr:?}");
                            reg.push_warning(format!(
                                "Rejecting self-recursive OR branch in {cpr:?}"
                            ));
                            continue;
                        } else {
                            // Allow non-recursive custom subcall as its own rule branch
                            reg.register(CustomRule {
                                pred: cpr.clone(),
                                head: head.clone(),
                                body: vec![st.clone()],
                                min_native_cost: 0,
                                min_subcall_count: 1,
                            });
                        }
                    }
                    Predicate::BatchSelf(_) | Predicate::Intro(_) => {
                        // Not supported in MVP
                        log::warn!(
                            "Skipping unsupported OR branch {:?} in {:?}",
                            st.pred(),
                            cpr
                        );
                        reg.push_warning(format!(
                            "Skipping unsupported OR branch {:?} in {:?}",
                            st.pred(),
                            cpr
                        ));
                        continue;
                    }
                }
            }
        }
    }
}
