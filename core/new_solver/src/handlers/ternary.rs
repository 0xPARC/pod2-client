use pod2::middleware::{StatementTmplArg, Value};
use tracing::trace;

use super::util::{classify_num, NumArg};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
    util::enumerate_choices_for,
};

pub struct TernaryArithmeticHandler {
    /// Given b and c, compute a.
    solve_for_a: fn(i64, i64) -> Option<i64>,
    /// Given a and c, compute b.
    solve_for_b: fn(i64, i64) -> Option<i64>,
    /// Given a and b, compute c.
    solve_for_c: fn(i64, i64) -> Option<i64>,
    op_name: &'static str,
}

impl TernaryArithmeticHandler {
    pub fn new(
        solve_for_a: fn(i64, i64) -> Option<i64>,
        solve_for_b: fn(i64, i64) -> Option<i64>,
        solve_for_c: fn(i64, i64) -> Option<i64>,
        op_name: &'static str,
    ) -> Self {
        Self {
            solve_for_a,
            solve_for_b,
            solve_for_c,
            op_name,
        }
    }
}

impl OpHandler for TernaryArithmeticHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }
        trace!(op = self.op_name, "Ternary: start");
        let a = classify_num(&args[0], store, edb);
        let b = classify_num(&args[1], store, edb);
        let c = classify_num(&args[2], store, edb);

        match (&a, &b, &c) {
            (NumArg::TypeError, _, _) | (_, NumArg::TypeError, _) | (_, _, NumArg::TypeError) => {
                return PropagatorResult::Contradiction;
            }
            (NumArg::NoFact, _, _) | (_, NumArg::NoFact, _) | (_, _, NumArg::NoFact) => {
                return PropagatorResult::Contradiction;
            }
            _ => {}
        }

        let mut grounds = 0;
        if let NumArg::Ground { .. } = a {
            grounds += 1;
        }
        if let NumArg::Ground { .. } = b {
            grounds += 1;
        }
        if let NumArg::Ground { .. } = c {
            grounds += 1;
        }

        if grounds < 2 {
            let mut waits = Vec::new();
            if let NumArg::Wait(w) = a {
                waits.push(w);
            }
            if let NumArg::Wait(w) = b {
                waits.push(w);
            }
            if let NumArg::Wait(w) = c {
                waits.push(w);
            }
            return if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            };
        }

        if grounds == 3 {
            // All ground: validate
            let (ai, pa) = if let NumArg::Ground { i, premises } = a {
                (i, premises)
            } else {
                unreachable!()
            };
            let (bi, pb) = if let NumArg::Ground { i, premises } = b {
                (i, premises)
            } else {
                unreachable!()
            };
            let (ci, pc) = if let NumArg::Ground { i, premises } = c {
                (i, premises)
            } else {
                unreachable!()
            };

            if (self.solve_for_a)(bi, ci) == Some(ai) {
                let mut premises = Vec::new();
                premises.extend(pa);
                premises.extend(pb);
                premises.extend(pc);
                return if premises.is_empty() {
                    PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::FromLiterals,
                    }
                } else {
                    PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::Derived { premises },
                    }
                };
            } else {
                return PropagatorResult::Contradiction;
            }
        }

        // Two-of-three binding
        match (&a, &b, &c) {
            (
                NumArg::Ground {
                    i: ai,
                    premises: pa,
                },
                NumArg::Ground {
                    i: bi,
                    premises: pb,
                },
                NumArg::Wait(wc_index),
            ) => {
                if let Some(target_i) = (self.solve_for_c)(*ai, *bi) {
                    let other_premises = {
                        let mut p = pa.clone();
                        p.extend(pb.clone());
                        p
                    };
                    if let StatementTmplArg::AnchoredKey(_w, key) = &args[2] {
                        let target_val = Value::from(target_i);
                        let mut choices = enumerate_choices_for(key, &target_val, *wc_index, edb);
                        if choices.is_empty() {
                            return PropagatorResult::Contradiction;
                        }

                        if !other_premises.is_empty() {
                            for choice in &mut choices {
                                if let OpTag::Derived { premises } = &mut choice.op_tag {
                                    premises.extend(other_premises.clone());
                                }
                            }
                        }
                        return PropagatorResult::Choices {
                            alternatives: choices,
                        };
                    } else {
                        // Is a Wildcard
                        return PropagatorResult::Entailed {
                            bindings: vec![(*wc_index, Value::from(target_i))],
                            op_tag: if other_premises.is_empty() {
                                OpTag::FromLiterals
                            } else {
                                OpTag::Derived {
                                    premises: other_premises,
                                }
                            },
                        };
                    }
                }
            }
            (
                NumArg::Ground {
                    i: ai,
                    premises: pa,
                },
                NumArg::Wait(wc_index),
                NumArg::Ground {
                    i: ci,
                    premises: pc,
                },
            ) => {
                if let Some(target_i) = (self.solve_for_b)(*ai, *ci) {
                    let other_premises = {
                        let mut p = pa.clone();
                        p.extend(pc.clone());
                        p
                    };
                    if let StatementTmplArg::AnchoredKey(_w, key) = &args[1] {
                        let target_val = Value::from(target_i);
                        let mut choices = enumerate_choices_for(key, &target_val, *wc_index, edb);
                        if choices.is_empty() {
                            return PropagatorResult::Contradiction;
                        }

                        if !other_premises.is_empty() {
                            for choice in &mut choices {
                                if let OpTag::Derived { premises } = &mut choice.op_tag {
                                    premises.extend(other_premises.clone());
                                }
                            }
                        }
                        return PropagatorResult::Choices {
                            alternatives: choices,
                        };
                    } else {
                        // Is a Wildcard
                        return PropagatorResult::Entailed {
                            bindings: vec![(*wc_index, Value::from(target_i))],
                            op_tag: if other_premises.is_empty() {
                                OpTag::FromLiterals
                            } else {
                                OpTag::Derived {
                                    premises: other_premises,
                                }
                            },
                        };
                    }
                }
            }
            (
                NumArg::Wait(wc_index),
                NumArg::Ground {
                    i: bi,
                    premises: pb,
                },
                NumArg::Ground {
                    i: ci,
                    premises: pc,
                },
            ) => {
                if let Some(target_i) = (self.solve_for_a)(*bi, *ci) {
                    let other_premises = {
                        let mut p = pb.clone();
                        p.extend(pc.clone());
                        p
                    };
                    if let StatementTmplArg::AnchoredKey(_w, key) = &args[0] {
                        let target_val = Value::from(target_i);
                        let mut choices = enumerate_choices_for(key, &target_val, *wc_index, edb);
                        if choices.is_empty() {
                            return PropagatorResult::Contradiction;
                        }

                        if !other_premises.is_empty() {
                            for choice in &mut choices {
                                if let OpTag::Derived { premises } = &mut choice.op_tag {
                                    premises.extend(other_premises.clone());
                                }
                            }
                        }
                        return PropagatorResult::Choices {
                            alternatives: choices,
                        };
                    } else {
                        // Is a Wildcard
                        return PropagatorResult::Entailed {
                            bindings: vec![(*wc_index, Value::from(target_i))],
                            op_tag: if other_premises.is_empty() {
                                OpTag::FromLiterals
                            } else {
                                OpTag::Derived {
                                    premises: other_premises,
                                }
                            },
                        };
                    }
                }
            }
            _ => {}
        }
        PropagatorResult::Contradiction
    }
}
