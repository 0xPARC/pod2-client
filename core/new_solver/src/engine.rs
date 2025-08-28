use pod2::middleware::{NativePredicate, StatementTmplArg};

use crate::{
    edb::EdbView,
    op::OpRegistry,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, FrameId},
};

#[derive(Clone, Debug)]
pub enum Frame {
    Producer {
        id: FrameId,
        /// Goals queued for evaluation: (predicate, template args)
        goals: Vec<(NativePredicate, Vec<StatementTmplArg>)>,
        store: ConstraintStore,
    },
    // Placeholder for future subgoal consumption
    Consumer {},
}

#[derive(Default)]
pub struct Scheduler {
    pub runnable: Vec<Frame>,
}

impl Scheduler {
    pub fn enqueue(&mut self, f: Frame) {
        self.runnable.push(f);
    }
    pub fn dequeue(&mut self) -> Option<Frame> {
        self.runnable.pop()
    }
}

pub struct Engine<'a> {
    pub registry: &'a OpRegistry,
    pub edb: &'a dyn EdbView,
    pub sched: Scheduler,
}

impl<'a> Engine<'a> {
    pub fn new(registry: &'a OpRegistry, edb: &'a dyn EdbView) -> Self {
        Self {
            registry,
            edb,
            sched: Scheduler::default(),
        }
    }

    pub fn run(&mut self) {
        while let Some(frame) = self.sched.dequeue() {
            match frame {
                Frame::Producer {
                    id,
                    mut goals,
                    store,
                } => {
                    if goals.is_empty() {
                        // Done: in a full system, emit an answer. For MVP we just drop.
                        continue;
                    }
                    let (goal_pred, tmpl_args) = goals.remove(0);
                    // Iterate handlers for this goal
                    let mut choices: Vec<Choice> = Vec::new();
                    for h in self.registry.get(goal_pred) {
                        let res = h.propagate(&tmpl_args, &mut store.clone(), self.edb);
                        match res {
                            PropagatorResult::Entailed { bindings, op_tag } => {
                                choices.push(Choice { bindings, op_tag })
                            }
                            PropagatorResult::Choices { mut alternatives } => {
                                choices.append(&mut alternatives)
                            }
                            PropagatorResult::Suspend { .. } => { /* MVP: ignore suspend */ }
                            PropagatorResult::Contradiction => {}
                        }
                    }
                    // De-dup choices by bindings; prefer GeneratedContains over Copy
                    if !choices.is_empty() {
                        use std::collections::HashMap;

                        use crate::types::OpTag;
                        let mut best: HashMap<
                            Vec<(usize, pod2::middleware::Value)>,
                            (i32, Choice),
                        > = HashMap::new();
                        for ch in choices.into_iter() {
                            let key = {
                                let mut b = ch.bindings.clone();
                                b.sort_by_key(|(i, _)| *i);
                                b
                            };
                            let score = match &ch.op_tag {
                                // Prefer any outcome that carries a GeneratedContains premise
                                OpTag::Derived { premises } => {
                                    if premises.iter().any(|(_, tag)| {
                                        matches!(tag, OpTag::GeneratedContains { .. })
                                    }) {
                                        3
                                    } else if premises
                                        .iter()
                                        .any(|(_, tag)| matches!(tag, OpTag::CopyStatement { .. }))
                                    {
                                        2
                                    } else {
                                        1
                                    }
                                }
                                OpTag::GeneratedContains { .. } => 3,
                                OpTag::CopyStatement { .. } => 2,
                                _ => 1,
                            };
                            match best.get_mut(&key) {
                                Some((best_score, _)) if *best_score >= score => {}
                                _ => {
                                    best.insert(key, (score, ch));
                                }
                            }
                        }
                        // Use the best choices
                        for (_k, (_s, ch)) in best.into_iter() {
                            let mut cont_store = store.clone();
                            for (w, v) in ch.bindings.iter().cloned() {
                                cont_store.bindings.insert(w, v);
                            }
                            let cont = Frame::Producer {
                                id,
                                goals: goals.clone(),
                                store: cont_store,
                            };
                            self.sched.enqueue(cont);
                        }
                        continue;
                    }
                    // No choices produced; nothing to enqueue for this goal.
                    for ch in std::iter::empty::<Choice>() {
                        let mut cont_store = store.clone();
                        for (w, v) in ch.bindings.iter().cloned() {
                            cont_store.bindings.insert(w, v);
                        }
                        // For MVP we do not instantiate/append premises yet; wire later.
                        let cont = Frame::Producer {
                            id,
                            goals: goals.clone(),
                            store: cont_store,
                        };
                        self.sched.enqueue(cont);
                    }
                }
                Frame::Consumer {} => {}
            }
        }
    }
}
