use pod2::middleware::{Predicate, StatementTmpl, StatementTmplArg};

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
        goals: Vec<StatementTmpl>,
        store: ConstraintStore,
    },
    // Placeholder for future subgoal consumption
    Consumer {},
}

#[derive(Default)]
pub struct Scheduler {
    pub runnable: Vec<Frame>,
    next_id: FrameId,
    // Suspension bookkeeping
    waitlist: std::collections::HashMap<usize, std::collections::HashSet<FrameId>>,
    parked: std::collections::HashMap<FrameId, ParkedFrame>,
}

impl Scheduler {
    pub fn enqueue(&mut self, f: Frame) {
        self.runnable.push(f);
    }
    pub fn dequeue(&mut self) -> Option<Frame> {
        self.runnable.pop()
    }
    pub fn new_id(&mut self) -> FrameId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn park(&mut self, frame: Frame, on: Vec<usize>, goal_stmt: StatementTmpl) {
        // Only producers are parkable in this MVP
        let (id, goals, store) = match frame {
            Frame::Producer {
                id,
                mut goals,
                store,
            } => {
                // Reinsert the suspended goal at the front so it retries on wake
                goals.insert(0, goal_stmt);
                (id, goals, store)
            }
            _ => return,
        };
        // Filter out already-bound wildcards
        let waiting_on: std::collections::HashSet<usize> = on
            .into_iter()
            .filter(|w| !store.bindings.contains_key(w))
            .collect();
        if waiting_on.is_empty() {
            // Nothing to wait on; just re-enqueue
            self.enqueue(Frame::Producer { id, goals, store });
            return;
        }
        // Index this parked frame under all waited wildcards
        for w in waiting_on.iter().cloned() {
            self.waitlist.entry(w).or_default().insert(id);
        }
        self.parked.insert(
            id,
            ParkedFrame {
                id,
                goals,
                store,
                waiting_on,
            },
        );
    }

    pub fn wake_with_bindings(
        &mut self,
        bindings: &[(usize, pod2::middleware::Value)],
    ) -> Vec<Frame> {
        use std::collections::HashSet;
        let mut runnable = Vec::new();
        let mut woken: HashSet<FrameId> = HashSet::new();
        // For each binding, wake frames waiting on this wildcard
        for (wid, val) in bindings.iter().cloned() {
            let ids: Vec<FrameId> = self
                .waitlist
                .get(&wid)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default();
            for id in ids {
                if let Some(mut pf) = self.parked.remove(&id) {
                    // Apply binding if compatible
                    let mut conflict = false;
                    match pf.store.bindings.get(&wid) {
                        Some(existing) if existing != &val => {
                            conflict = true;
                        }
                        _ => {
                            pf.store.bindings.insert(wid, val.clone());
                            pf.waiting_on.remove(&wid);
                        }
                    }
                    // Clean all registrations for this frame id from waitlist (we will re-park if it suspends again)
                    let remaining_keys: Vec<usize> = pf.waiting_on.iter().cloned().collect();
                    for k in remaining_keys {
                        if let Some(set) = self.waitlist.get_mut(&k) {
                            set.remove(&id);
                        }
                    }
                    if !conflict && woken.insert(id) {
                        runnable.push(Frame::Producer {
                            id: pf.id,
                            goals: pf.goals,
                            store: pf.store,
                        });
                    }
                }
                // Remove id from this wid's waitlist set
                if let Some(set) = self.waitlist.get_mut(&wid) {
                    set.remove(&id);
                    if set.is_empty() {
                        self.waitlist.remove(&wid);
                    }
                }
            }
        }
        runnable
    }
}

#[derive(Clone, Debug)]
struct ParkedFrame {
    id: FrameId,
    goals: Vec<StatementTmpl>,
    store: ConstraintStore,
    waiting_on: std::collections::HashSet<usize>,
}

pub struct Engine<'a> {
    pub registry: &'a OpRegistry,
    pub edb: &'a dyn EdbView,
    pub sched: Scheduler,
    pub answers: Vec<crate::types::ConstraintStore>,
}

impl<'a> Engine<'a> {
    pub fn new(registry: &'a OpRegistry, edb: &'a dyn EdbView) -> Self {
        Self {
            registry,
            edb,
            sched: Scheduler::default(),
            answers: Vec::new(),
        }
    }

    pub fn run(&mut self) {
        while let Some(frame) = self.sched.dequeue() {
            match frame {
                Frame::Producer { id, goals, store } => {
                    if goals.is_empty() {
                        // Record a completed answer (bindings and any accumulated premises)
                        self.answers.push(store.clone());
                        continue;
                    }
                    // Evaluate goals sequentially; branch on the first goal that yields choices.
                    let mut chosen_goal_idx: Option<usize> = None;
                    let mut choices_for_goal: Vec<Choice> = Vec::new();
                    let mut union_waits: std::collections::HashSet<usize> =
                        std::collections::HashSet::new();
                    let mut any_stmt_for_park: Option<StatementTmpl> = None;
                    for (idx, g) in goals.iter().enumerate() {
                        let tmpl_args: Vec<StatementTmplArg> = g.args.clone();
                        let goal_pred = match g.pred {
                            Predicate::Native(p) => p,
                            Predicate::Custom(_)
                            | Predicate::BatchSelf(_)
                            | Predicate::Intro(_) => continue,
                        };
                        let mut local_choices: Vec<Choice> = Vec::new();
                        let mut suspended_here = false;
                        for h in self.registry.get(goal_pred) {
                            match h.propagate(&tmpl_args, &mut store.clone(), self.edb) {
                                PropagatorResult::Entailed { bindings, op_tag } => {
                                    local_choices.push(Choice { bindings, op_tag })
                                }
                                PropagatorResult::Choices { mut alternatives } => {
                                    local_choices.append(&mut alternatives)
                                }
                                PropagatorResult::Suspend { on } => {
                                    suspended_here = true;
                                    if any_stmt_for_park.is_none() {
                                        any_stmt_for_park = Some(g.clone());
                                    }
                                    for w in on {
                                        if !store.bindings.contains_key(&w) {
                                            union_waits.insert(w);
                                        }
                                    }
                                }
                                PropagatorResult::Contradiction => {}
                            }
                        }
                        if !local_choices.is_empty() {
                            chosen_goal_idx = Some(idx);
                            choices_for_goal = local_choices;
                            break;
                        } else {
                            let _ = suspended_here;
                        }
                    }
                    if choices_for_goal.is_empty() {
                        // No immediate choices; if any suspensions, park frame on their union
                        if !union_waits.is_empty() {
                            let on: Vec<usize> = union_waits.into_iter().collect();
                            let stmt_for_park =
                                any_stmt_for_park.unwrap_or_else(|| goals[0].clone());
                            self.sched.park(
                                Frame::Producer {
                                    id,
                                    goals: goals.clone(),
                                    store: store.clone(),
                                },
                                on,
                                stmt_for_park,
                            );
                            continue;
                        } else {
                            // No choices and no suspends → no progress possible; drop frame
                            continue;
                        }
                    }
                    // De-dup choices by bindings; prefer GeneratedContains over Copy
                    if !choices_for_goal.is_empty() {
                        use std::collections::HashMap;

                        use crate::types::OpTag;
                        let mut best: HashMap<
                            Vec<(usize, pod2::middleware::Value)>,
                            (i32, Choice),
                        > = HashMap::new();
                        for ch in choices_for_goal.into_iter() {
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
                            // Wake any parked frames that were waiting on these bindings
                            for woke in self.sched.wake_with_bindings(&ch.bindings) {
                                self.sched.enqueue(woke);
                            }
                            let mut ng = goals.clone();
                            if let Some(i) = chosen_goal_idx {
                                ng.remove(i);
                            }
                            // Record head proof step for this goal in the continuation store
                            if let Some(i) = chosen_goal_idx {
                                let head_tmpl = &goals[i];
                                if let Some(head) =
                                    crate::util::instantiate_goal(head_tmpl, &cont_store.bindings)
                                {
                                    cont_store.premises.push((head, ch.op_tag.clone()));
                                }
                            }
                            let cont = Frame::Producer {
                                id: self.sched.new_id(),
                                goals: ng,
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
                        let mut ng = goals.clone();
                        if let Some(i) = chosen_goal_idx {
                            ng.remove(i);
                        }
                        let cont = Frame::Producer {
                            id,
                            goals: ng,
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

#[cfg(test)]
mod tests {
    use pod2::{
        lang::parse,
        middleware::{containers::Dictionary, Key, Params, Value},
    };

    use super::*;
    use crate::{
        edb::MockEdbView,
        handlers::{register_equal_handlers, register_lt_handlers},
        op::OpRegistry,
        types::ConstraintStore,
    };

    #[test]
    fn engine_solves_two_goals_with_shared_root() {
        // Build a full dictionary with k:1, x:5 so both goals can be satisfied by same root
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [
                (Key::from("k"), Value::from(1)),
                (Key::from("x"), Value::from(5)),
            ]
            .into(),
        )
        .unwrap();
        let root = dict.commitment();
        let mut edb = MockEdbView::default();
        edb.add_full_dict(dict);

        // Registry with Equal and Lt handlers
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);

        // Build goals via parser: Equal(?R["k"], 1) and Lt(?R["x"], 10)
        let processed = parse(
            r#"REQUEST(
                Equal(?R["k"], 1)
                Lt(?R["x"], 10)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let goals = processed.request.templates().to_vec();

        let mut engine = Engine::new(&reg, &edb);
        let id0 = engine.sched.new_id();
        engine.sched.enqueue(Frame::Producer {
            id: id0,
            goals,
            store: ConstraintStore::default(),
        });
        engine.run();

        assert!(!engine.answers.is_empty());
        // At least one answer should bind wildcard 0 to the correct root
        let any_matches = engine.answers.iter().any(|store| {
            store
                .bindings
                .get(&0)
                .map(|v| v.raw() == Value::from(root).raw())
                .unwrap_or(false)
        });
        assert!(any_matches, "no answer bound ?R to the expected root");

        // Check that premises include Equal(R["k"],1) and Lt(R["x"],10)
        use pod2::middleware::{AnchoredKey, Statement, ValueRef};
        let mut saw_equal = false;
        let mut saw_lt = false;
        for st in engine.answers.iter() {
            for (stmt, tag) in st.premises.iter() {
                match stmt {
                    Statement::Equal(
                        ValueRef::Key(AnchoredKey { root: r, key }),
                        ValueRef::Literal(v),
                    ) => {
                        if *r == root && key.name() == "k" && *v == Value::from(1) {
                            saw_equal = true;
                            // EqualFromEntries should be Derived with a Contains premise
                            assert!(matches!(tag, crate::types::OpTag::Derived { .. }));
                        }
                    }
                    Statement::Lt(
                        ValueRef::Key(AnchoredKey { root: r, key }),
                        ValueRef::Literal(v),
                    ) => {
                        if *r == root && key.name() == "x" && *v == Value::from(10) {
                            saw_lt = true;
                            assert!(matches!(tag, crate::types::OpTag::Derived { .. }));
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_equal && saw_lt,
            "expected Equal and Lt proof steps recorded"
        );
    }

    // Suspend/park/wake integration tests will be added after broader wakeup wiring.
    #[test]
    fn engine_single_frame_intra_fixpoint() {
        // First goal suspends (Lt on AK with unbound root), second goal binds the root; then Lt succeeds without parking.
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [
                (Key::from("k"), Value::from(1)),
                (Key::from("x"), Value::from(5)),
            ]
            .into(),
        )
        .unwrap();
        let root = dict.commitment();
        let mut edb = MockEdbView::default();
        edb.add_full_dict(dict);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);

        // Lt first (suspends), Equal second (binds root)
        let processed = parse(
            r#"REQUEST(
                Lt(?R["x"], 10)
                Equal(?R["k"], 1)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let goals = processed.request.templates().to_vec();

        let mut engine = Engine::new(&reg, &edb);
        let id0 = engine.sched.new_id();
        engine.sched.enqueue(Frame::Producer {
            id: id0,
            goals,
            store: ConstraintStore::default(),
        });
        engine.run();

        // Should have reached an answer without leaving parked frames
        assert!(engine.sched.parked.is_empty(), "frame should not be parked");
        assert!(!engine.answers.is_empty(), "expected an answer");
        let any_matches = engine.answers.iter().any(|store| {
            store
                .bindings
                .get(&0)
                .map(|v| v.raw() == Value::from(root).raw())
                .unwrap_or(false)
        });
        assert!(any_matches, "no answer bound ?R to expected root");

        // Check that premises include both steps
        use pod2::middleware::{AnchoredKey, Statement, ValueRef};
        let mut saw_equal = false;
        let mut saw_lt = false;
        for st in engine.answers.iter() {
            for (stmt, tag) in st.premises.iter() {
                match stmt {
                    Statement::Equal(
                        ValueRef::Key(AnchoredKey { root: r, key }),
                        ValueRef::Literal(v),
                    ) => {
                        if *r == root && key.name() == "k" && *v == Value::from(1) {
                            saw_equal = true;
                            assert!(matches!(tag, crate::types::OpTag::Derived { .. }));
                        }
                    }
                    Statement::Lt(
                        ValueRef::Key(AnchoredKey { root: r, key }),
                        ValueRef::Literal(v),
                    ) => {
                        if *r == root && key.name() == "x" && *v == Value::from(10) {
                            saw_lt = true;
                            assert!(matches!(tag, crate::types::OpTag::Derived { .. }));
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_equal && saw_lt,
            "expected Equal and Lt proof steps recorded"
        );
    }

    #[test]
    fn engine_single_frame_suspends_when_no_progress() {
        // Single goal: Lt(?R["x"], 10) with no other goal to bind ?R → should park the frame
        let edb = MockEdbView::default();
        let mut reg = OpRegistry::default();
        register_lt_handlers(&mut reg);
        let processed = parse(
            r#"REQUEST(
                Lt(?R["x"], 10)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let goals = processed.request.templates().to_vec();

        let mut engine = Engine::new(&reg, &edb);
        let id0 = engine.sched.new_id();
        engine.sched.enqueue(Frame::Producer {
            id: id0,
            goals,
            store: ConstraintStore::default(),
        });
        engine.run();

        assert!(engine.answers.is_empty(), "should not produce an answer");
        assert_eq!(
            engine.sched.parked.len(),
            1,
            "frame should be parked waiting on ?R"
        );
    }

    #[test]
    fn engine_prefers_generated_contains_over_copy_for_same_binding() {
        // Setup a root with k:1 available both via copied Contains and via full dictionary
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("k"), Value::from(1))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let mut edb = MockEdbView::default();
        // Register both sources for the same (root, key, value)
        edb.add_copied_contains(
            root,
            Key::from("k"),
            Value::from(1),
            crate::types::PodRef(root),
        );
        edb.add_full_dict(dict);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // Single goal Equal(?R["k"], 1) should bind ?R to root. Two internal choices exist;
        // engine must dedup and prefer the GeneratedContains-based proof.
        let processed = parse(
            r#"REQUEST(
                Equal(?R["k"], 1)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let goals = processed.request.templates().to_vec();

        let mut engine = Engine::new(&reg, &edb);
        let id0 = engine.sched.new_id();
        engine.sched.enqueue(Frame::Producer {
            id: id0,
            goals,
            store: ConstraintStore::default(),
        });
        engine.run();

        assert_eq!(engine.answers.len(), 1);
        let st = &engine.answers[0];
        // Binding should be to the expected root
        assert_eq!(
            st.bindings.get(&0).map(|v| v.raw()),
            Some(Value::from(root).raw())
        );
        // Check that the recorded head proof step carries a GeneratedContains premise
        use pod2::middleware::{AnchoredKey, Statement, ValueRef};
        let mut saw_gen = false;
        for (stmt, tag) in st.premises.iter() {
            if let Statement::Equal(
                ValueRef::Key(AnchoredKey { root: r, key }),
                ValueRef::Literal(v),
            ) = stmt
            {
                if *r == root && key.name() == "k" && *v == Value::from(1) {
                    if let crate::types::OpTag::Derived { premises } = tag {
                        if premises.iter().any(|(_, pt)| {
                            matches!(pt, crate::types::OpTag::GeneratedContains { .. })
                        }) {
                            saw_gen = true;
                        }
                    }
                }
            }
        }
        assert!(
            saw_gen,
            "expected GeneratedContains premise to be preferred"
        );
    }
}
