use pod2::middleware::{Predicate, StatementTmpl, StatementTmplArg};

use crate::{
    custom::{remap_arg, remap_tmpl, CustomRule, RuleRegistry},
    edb::EdbView,
    op::OpRegistry,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, FrameId, PendingCustom},
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
    pub runnable: std::collections::VecDeque<Frame>,
    next_id: FrameId,
    // Suspension bookkeeping
    waitlist: std::collections::BTreeMap<usize, std::collections::BTreeSet<FrameId>>,
    parked: std::collections::HashMap<FrameId, ParkedFrame>,
}

impl Scheduler {
    pub fn enqueue(&mut self, f: Frame) {
        self.runnable.push_back(f);
    }
    pub fn dequeue(&mut self, policy: SchedulePolicy) -> Option<Frame> {
        match policy {
            SchedulePolicy::DepthFirst => self.runnable.pop_back(),
            SchedulePolicy::BreadthFirst => self.runnable.pop_front(),
        }
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
        // Index this parked frame under all waited wildcards (ordered)
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
        let mut sorted_bindings = bindings.to_vec();
        sorted_bindings.sort_by_key(|(w, _)| *w);
        for (wid, val) in sorted_bindings.into_iter() {
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
    pub rules: RuleRegistry,
    pub policy: SchedulePolicy,
}

impl<'a> Engine<'a> {
    pub fn new(registry: &'a OpRegistry, edb: &'a dyn EdbView) -> Self {
        Self {
            registry,
            edb,
            sched: Scheduler::default(),
            answers: Vec::new(),
            rules: RuleRegistry::default(),
            policy: SchedulePolicy::DepthFirst,
        }
    }

    pub fn with_policy(
        registry: &'a OpRegistry,
        edb: &'a dyn EdbView,
        policy: SchedulePolicy,
    ) -> Self {
        let mut e = Self::new(registry, edb);
        e.policy = policy;
        e
    }

    /// Convenience: load a parsed Podlang program (custom predicates + request),
    /// register its custom predicates as conjunctive rules, and enqueue the request goals.
    pub fn load_processed(&mut self, processed: &pod2::lang::processor::PodlangOutput) {
        crate::custom::register_rules_from_batch(&mut self.rules, &processed.custom_batch);
        let goals = processed.request.templates().to_vec();
        let id0 = self.sched.new_id();
        self.sched.enqueue(Frame::Producer {
            id: id0,
            goals,
            store: ConstraintStore::default(),
        });
    }

    pub fn run(&mut self) {
        while let Some(frame) = self.sched.dequeue(self.policy) {
            match frame {
                Frame::Producer { id, goals, store } => {
                    if goals.is_empty() {
                        // Record a completed answer (bindings and any accumulated premises)
                        let mut final_store = store.clone();
                        // Materialize any pending custom deductions as head proof steps
                        if !final_store.pending_custom.is_empty() {
                            let pendings = final_store.pending_custom.clone();
                            for p in pendings.into_iter() {
                                if let Some(head) = crate::util::instantiate_custom(
                                    &p.rule_id,
                                    &p.head_args,
                                    &final_store.bindings,
                                ) {
                                    let premises = final_store.premises.clone();
                                    final_store.premises.push((
                                        head,
                                        crate::types::OpTag::CustomDeduction {
                                            rule_id: p.rule_id.clone(),
                                            premises,
                                        },
                                    ));
                                }
                            }
                            final_store.pending_custom.clear();
                        }
                        self.answers.push(final_store);
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
                        // Handle native vs custom
                        let is_custom = matches!(g.pred, Predicate::Custom(_));
                        if is_custom {
                            // Expand simple conjunctive custom rules (non-recursive, single-head) into body goals
                            if let Predicate::Custom(ref cpr) = g.pred {
                                let rules = self.rules.get(cpr).to_vec();
                                if !rules.is_empty() {
                                    let mut produced_any = false;
                                    for rule in rules.iter() {
                                        if let Some(cont) = self
                                            .expand_custom_rule(id, &goals, &store, idx, cpr, rule)
                                        {
                                            self.sched.enqueue(cont);
                                            produced_any = true;
                                        }
                                    }
                                    if produced_any {
                                        chosen_goal_idx = Some(idx);
                                        choices_for_goal = Vec::new();
                                        break;
                                    }
                                }
                            }
                            continue;
                        }
                        let goal_pred = match g.pred {
                            Predicate::Native(p) => p,
                            _ => unreachable!(),
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
                    // De-dup choices by bindings; prefer GeneratedContains over Copy (deterministic order)
                    if !choices_for_goal.is_empty() {
                        use std::collections::BTreeMap;

                        use crate::types::OpTag;
                        // Stable map keyed by a canonical string of bindings
                        let mut best: BTreeMap<String, (i32, Choice)> = BTreeMap::new();
                        for ch in choices_for_goal.into_iter() {
                            let mut b = ch.bindings.clone();
                            b.sort_by_key(|(i, _)| *i);
                            let key = {
                                let mut s = String::new();
                                for (i, v) in b.iter() {
                                    use hex::ToHex;
                                    s.push_str(&format!("{}:", i));
                                    let raw = v.raw();
                                    s.push_str(&format!("{}|", raw.encode_hex::<String>()));
                                }
                                s
                            };
                            let score = match &ch.op_tag {
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
                        // Use the best choices in a stable order
                        for (_key, (_score, ch)) in best.into_iter() {
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

    /// Expand a custom rule into a continuation frame: bind head args against call args,
    /// remap rule-local wildcards to the current frame index space, and push body goals.
    fn expand_custom_rule(
        &mut self,
        frame_id: FrameId,
        goals: &[StatementTmpl],
        store: &ConstraintStore,
        goal_idx: usize,
        cpr: &pod2::middleware::CustomPredicateRef,
        rule: &CustomRule,
    ) -> Option<Frame> {
        // For MVP, require rule head arity matches and head args are Wildcards.
        if rule.head.len() != goals[goal_idx].args.len() {
            return None;
        }
        // Build mapping from rule wildcard indices to outer frame wildcard indices.
        use std::collections::HashMap;
        let mut map: HashMap<usize, usize> = HashMap::new();
        let mut next_idx = self.next_available_wildcard_index(goals, store) + 1;
        let call_args = &goals[goal_idx].args;

        // Seed mapping from head
        for (h, call) in rule.head.iter().zip(call_args.iter()) {
            match (h, call) {
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Wildcard(cw)) => {
                    map.insert(hw.index, cw.index);
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::AnchoredKey(cw, _)) => {
                    map.insert(hw.index, cw.index);
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Literal(v)) => {
                    // Allocate a fresh wildcard to hold this literal binding
                    let target = next_idx;
                    map.insert(hw.index, target);
                    next_idx += 1;
                    // We will apply this binding in the continuation store below
                }
                _ => {
                    // For MVP, don't support non-wildcard heads
                    return None;
                }
            }
        }

        // Remap head args and body
        let remapped_head: Vec<StatementTmplArg> =
            rule.head.iter().map(|a| remap_arg(a, &map)).collect();
        let remapped_body: Vec<StatementTmpl> =
            rule.body.iter().map(|t| remap_tmpl(t, &map)).collect();

        // Build continuation store and apply literal bindings from call args
        let mut cont_store = store.clone();
        for (h, call) in remapped_head.iter().zip(call_args.iter()) {
            if let (StatementTmplArg::Wildcard(hw), StatementTmplArg::Literal(v)) = (h, call) {
                cont_store.bindings.insert(hw.index, v.clone());
            }
        }
        // Build new goals: propagate allowed parent constraints, then body, then remaining goals
        use pod2::middleware::NativePredicate;
        // Head wildcard indices (outer frame) referenced by the call
        let mut head_wcs: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for a in call_args.iter() {
            match a {
                StatementTmplArg::Wildcard(w) => {
                    head_wcs.insert(w.index);
                }
                StatementTmplArg::AnchoredKey(w, _) => {
                    head_wcs.insert(w.index);
                }
                _ => {}
            }
        }
        // Collect indices of parent goals to propagate (monotonic natives over head vars)
        let mut propagate_idxs: Vec<usize> = Vec::new();
        for (i, g) in goals.iter().enumerate() {
            if i == goal_idx {
                continue;
            }
            let pred_ok = matches!(
                g.pred,
                Predicate::Native(NativePredicate::Lt)
                    | Predicate::Native(NativePredicate::LtEq)
                    | Predicate::Native(NativePredicate::NotContains)
            );
            if !pred_ok {
                continue;
            }
            // All wildcards must be within head set
            let wcs = crate::prop::wildcards_in_args(&g.args);
            if wcs.iter().all(|w| head_wcs.contains(w)) {
                propagate_idxs.push(i);
            }
        }

        let mut ng =
            Vec::with_capacity(goals.len() - 1 + remapped_body.len() + propagate_idxs.len());
        // Prepend propagated constraints (de-dup if already present in body)
        let mut propagated_count = 0usize;
        for i in propagate_idxs.iter().cloned() {
            let g = goals[i].clone();
            if remapped_body.contains(&g) {
                continue;
            }
            propagated_count += 1;
            ng.push(g);
        }
        // Then the body
        ng.extend(remapped_body);
        // Then remaining parent goals excluding the custom goal and any propagated ones
        for (i, g) in goals.iter().enumerate() {
            if i != goal_idx && !propagate_idxs.contains(&i) {
                ng.push(g.clone());
            }
        }
        if propagated_count > 0 {
            log::debug!(
                "Propagated {} calling-context constraints into {:?}",
                propagated_count,
                cpr
            );
        }
        // Push a pending custom deduction to materialize on success
        cont_store.pending_custom.push(PendingCustom {
            rule_id: cpr.clone(),
            head_args: remapped_head,
        });

        Some(Frame::Producer {
            id: self.sched.new_id(),
            goals: ng,
            store: cont_store,
        })
    }

    fn next_available_wildcard_index(
        &self,
        goals: &[StatementTmpl],
        store: &ConstraintStore,
    ) -> usize {
        let mut max_idx = 0usize;
        for g in goals.iter() {
            for a in g.args.iter() {
                match a {
                    StatementTmplArg::Wildcard(w) => max_idx = max_idx.max(w.index),
                    StatementTmplArg::AnchoredKey(w, _) => max_idx = max_idx.max(w.index),
                    _ => {}
                }
            }
        }
        for k in store.bindings.keys() {
            max_idx = max_idx.max(*k);
        }
        max_idx
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SchedulePolicy {
    DepthFirst,
    BreadthFirst,
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
        handlers::{
            lteq::register_lteq_handlers, register_contains_handlers, register_equal_handlers,
            register_lt_handlers, register_sumof_handlers,
        },
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
        crate::handlers::lteq::register_lteq_handlers(&mut reg);

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

    #[test]
    fn scheduler_policy_depth_first_vs_breadth_first() {
        // Build two trivial frames with prepopulated bindings and no goals; check answer order
        let edb = MockEdbView::default();
        let reg = OpRegistry::default();

        // Depth-first (default): last enqueued should be answered first
        let mut eng_dfs = Engine::new(&reg, &edb);
        let mut s1 = ConstraintStore::default();
        s1.bindings.insert(0, Value::from(10));
        let mut s2 = ConstraintStore::default();
        s2.bindings.insert(0, Value::from(20));
        let id_a = eng_dfs.sched.new_id();
        eng_dfs.sched.enqueue(Frame::Producer {
            id: id_a,
            goals: vec![],
            store: s1,
        });
        let id_b = eng_dfs.sched.new_id();
        eng_dfs.sched.enqueue(Frame::Producer {
            id: id_b,
            goals: vec![],
            store: s2,
        });
        eng_dfs.run();
        assert_eq!(eng_dfs.answers.len(), 2);
        // First answer should be from s2 (20)
        assert_eq!(eng_dfs.answers[0].bindings.get(&0), Some(&Value::from(20)));
        assert_eq!(eng_dfs.answers[1].bindings.get(&0), Some(&Value::from(10)));

        // Breadth-first: first enqueued should be answered first
        let mut eng_bfs = Engine::with_policy(&reg, &edb, SchedulePolicy::BreadthFirst);
        let mut t1 = ConstraintStore::default();
        t1.bindings.insert(0, Value::from(1));
        let mut t2 = ConstraintStore::default();
        t2.bindings.insert(0, Value::from(2));
        let id_c = eng_bfs.sched.new_id();
        eng_bfs.sched.enqueue(Frame::Producer {
            id: id_c,
            goals: vec![],
            store: t1,
        });
        let id_d = eng_bfs.sched.new_id();
        eng_bfs.sched.enqueue(Frame::Producer {
            id: id_d,
            goals: vec![],
            store: t2,
        });
        eng_bfs.run();
        assert_eq!(eng_bfs.answers.len(), 2);
        assert_eq!(eng_bfs.answers[0].bindings.get(&0), Some(&Value::from(1)));
        assert_eq!(eng_bfs.answers[1].bindings.get(&0), Some(&Value::from(2)));
    }

    #[test]
    fn determinism_golden_many_choices() {
        // Build 5 roots each with k:1; query Equal(?R["k"], 1). Ordering should be stable across runs.
        let params = Params::default();
        let mut edb = MockEdbView::default();
        let mut roots = Vec::new();
        for i in 0..5 {
            let key = Key::from("k");
            let dict = Dictionary::new(
                params.max_depth_mt_containers,
                [(key.clone(), Value::from(1))].into(),
            )
            .unwrap();
            let r = dict.commitment();
            edb.add_full_dict(dict);
            roots.push(r);
        }

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let processed = parse(
            r#"REQUEST(
                Equal(?R["k"], 1)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let goals = processed.request.templates().to_vec();

        // First run
        let mut engine1 = Engine::new(&reg, &edb);
        let id1 = engine1.sched.new_id();
        engine1.sched.enqueue(Frame::Producer {
            id: id1,
            goals: goals.clone(),
            store: ConstraintStore::default(),
        });
        engine1.run();
        let seq1: Vec<_> = engine1
            .answers
            .iter()
            .filter_map(|st| st.bindings.get(&0).cloned())
            .map(|v| pod2::middleware::Hash::from(v.raw()))
            .collect();

        // Second run
        let mut engine2 = Engine::new(&reg, &edb);
        let id2 = engine2.sched.new_id();
        engine2.sched.enqueue(Frame::Producer {
            id: id2,
            goals,
            store: ConstraintStore::default(),
        });
        engine2.run();
        let seq2: Vec<_> = engine2
            .answers
            .iter()
            .filter_map(|st| st.bindings.get(&0).cloned())
            .map(|v| pod2::middleware::Hash::from(v.raw()))
            .collect();

        assert_eq!(
            seq1, seq2,
            "Answer order should be deterministic across runs"
        );
        // And the sequence should be sorted by root (as per EDB stable ordering and choice ordering)
        let mut sorted = seq1.clone();
        sorted.sort();
        assert_eq!(
            seq1, sorted,
            "Expected answers ordered by increasing root hash"
        );
    }

    #[test]
    fn engine_propagates_calling_context_constraints_into_subcall() {
        // Parent has Lt(?A, 20); subcall binds ?A via Equal from entries
        let params = Params::default();
        let mut edb = MockEdbView::default();
        // Two dicts: one satisfies A=15 (<20), another violates A=25
        let d_ok = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(15))].into(),
        )
        .unwrap();
        let d_bad = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(25))].into(),
        )
        .unwrap();
        let r_ok = d_ok.commitment();
        let r_bad = d_bad.commitment();
        edb.add_full_dict(d_ok);
        edb.add_full_dict(d_bad);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);
        register_lteq_handlers(&mut reg);

        // Define helper AND that ties A to R["x"], then call it under top-level Lt(?A,20)
        // and Equal(?R["x"], 15) to ground the subcall.
        let input = r#"
            helper(A, R) = AND(
                Equal(?R["x"], ?A)
            )

            REQUEST(
                Lt(?A, 20)
                Equal(?R["x"], 15)
                helper(?A, ?R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run();

        // Expect at least one answer with (A=15, R=r_ok) and no answer with R=r_bad
        let has_ok = engine.answers.iter().any(|st| {
            st.bindings.get(&0) == Some(&Value::from(15))
                && st.bindings.get(&1).map(|v| v.raw()) == Some(Value::from(r_ok).raw())
        });
        assert!(has_ok, "expected an answer with A=15 and R=r_ok");
        let has_bad = engine
            .answers
            .iter()
            .any(|st| st.bindings.get(&1).map(|v| v.raw()) == Some(Value::from(r_bad).raw()));
        assert!(!has_bad, "should not bind R to r_bad");
    }

    #[test]
    fn engine_does_not_propagate_constraints_with_private_vars() {
        // A parent constraint mentioning a non-head wildcard should not be propagated
        let params = Params::default();
        let mut edb = MockEdbView::default();
        let d = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(10))].into(),
        )
        .unwrap();
        edb.add_full_dict(d);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);

        // The Lt(?Z, 5) constraint mentions ?Z which is not in helper's head → must not be propagated
        let input = r#"
            helper(A, R) = AND(
                Equal(?R["x"], ?A)
            )

            REQUEST(
                Lt(?Z, 5)
                helper(?A, ?R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        // Register rules but don't enqueue request yet
        crate::custom::register_rules_from_batch(&mut engine.rules, &processed.custom_batch);
        // Build parent goals vector
        let parent_goals = processed.request.templates().to_vec();
        // Locate the predicate ref
        let cpr = if let Predicate::Custom(ref c) = parent_goals[1].pred {
            c.clone()
        } else {
            panic!("expected custom")
        };
        let rules = engine.rules.get(&cpr).to_vec();
        assert!(!rules.is_empty());
        // Expand the custom rule
        let frame = engine
            .expand_custom_rule(
                0,
                &parent_goals,
                &ConstraintStore::default(),
                1,
                &cpr,
                &rules[0],
            )
            .expect("frame");
        // The first goal should be the body Equal, not the unrelated Lt(?Z,5)
        if let Frame::Producer { goals, .. } = frame {
            // The propagated list should not include Lt(?Z,5) since Z is not in helper head
            use pod2::middleware::NativePredicate;
            if let Predicate::Native(NativePredicate::Lt) = goals[0].pred {
                panic!("unexpected propagation of private Lt");
            }
        } else {
            panic!("unexpected frame kind");
        }
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

    #[test]
    fn engine_custom_conjunctive_rule_end_to_end() {
        use pod2::middleware::CustomPredicateRef;

        let params = Params::default();
        // EDB: R has some_key:20; C has other_key:20
        let mut edb = MockEdbView::default();
        let dict_r = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("some_key"), Value::from(20))].into(),
        )
        .unwrap();
        let dict_c = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("other_key"), Value::from(20))].into(),
        )
        .unwrap();
        let root_r = dict_r.commitment();
        let root_c = dict_c.commitment();
        edb.add_full_dict(dict_r);
        edb.add_full_dict(dict_c);

        // Registry with all needed native handlers
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);
        crate::register_lteq_handlers(&mut reg);
        crate::register_not_contains_handlers(&mut reg);
        register_sumof_handlers(&mut reg);
        register_contains_handlers(&mut reg);
        // Alternative path: define predicate and request in a single Podlang program
        let input = r#"
            my_pred(A, R, C) = AND(
                Lt(?A, 50)
                Equal(?R["some_key"], ?A)
                Equal(?C["other_key"], ?A)
                SumOf(?R["some_key"], 19, 1)
            )

            REQUEST(
                my_pred(?A, ?R, ?C)
            )
        "#;
        let processed2 = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        // Load and enqueue via helper
        engine.load_processed(&processed2);
        let cpr = CustomPredicateRef::new(processed2.custom_batch.clone(), 0);
        engine.run();

        assert_eq!(engine.answers.len(), 1);
        let ans = &engine.answers[0];
        // Check bindings
        assert_eq!(ans.bindings.get(&0), Some(&Value::from(20))); // A = 20
        assert_eq!(
            ans.bindings.get(&1).map(|v| v.raw()),
            Some(Value::from(root_r).raw())
        );
        assert_eq!(
            ans.bindings.get(&2).map(|v| v.raw()),
            Some(Value::from(root_c).raw())
        );

        // Check that a CustomDeduction head was recorded
        use pod2::middleware::Statement;
        let mut saw_custom = false;
        for (stmt, tag) in ans.premises.iter() {
            if let Statement::Custom(pred, vals) = stmt {
                if *pred == cpr {
                    assert_eq!(vals.len(), 3);
                    assert_eq!(vals[0], Value::from(20));
                    assert_eq!(vals[1].raw(), Value::from(root_r).raw());
                    assert_eq!(vals[2].raw(), Value::from(root_c).raw());
                    if let crate::types::OpTag::CustomDeduction { .. } = tag {
                        saw_custom = true;
                    }
                }
            }
        }
        assert!(saw_custom, "expected CustomDeduction head in premises");
    }

    #[test]
    fn engine_custom_or_rule_enumerates_roots() {
        use pod2::middleware::CustomPredicateRef;

        let params = Params::default();
        // EDB: two roots with a:1 and a:2 respectively
        let mut edb = MockEdbView::default();
        let d1 = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("a"), Value::from(1))].into(),
        )
        .unwrap();
        let r1 = d1.commitment();
        edb.add_full_dict(d1);
        let d2 = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("a"), Value::from(2))].into(),
        )
        .unwrap();
        let r2 = d2.commitment();
        edb.add_full_dict(d2);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // Define disjunctive predicate and request
        let input = r#"
            my_pred(R) = OR(
                Equal(?R["a"], 1)
                Equal(?R["a"], 2)
            )

            REQUEST(
                my_pred(?R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        let cpr = CustomPredicateRef::new(processed.custom_batch.clone(), 0);
        engine.run();

        // Expect two answers binding ?R to r1 and r2
        let roots: std::collections::HashSet<_> = engine
            .answers
            .iter()
            .filter_map(|st| st.bindings.get(&0).cloned())
            .map(|v| pod2::middleware::Hash::from(v.raw()))
            .collect();
        assert!(roots.contains(&r1) && roots.contains(&r2));

        // Each answer should include a CustomDeduction head for my_pred
        use pod2::middleware::Statement;
        for st in engine.answers.iter() {
            assert!(st.premises.iter().any(|(stmt, tag)| {
                match stmt {
                    Statement::Custom(pred, _vals) if *pred == cpr => {
                        matches!(tag, crate::types::OpTag::CustomDeduction { .. })
                    }
                    _ => false,
                }
            }));
        }
    }

    #[test]
    fn engine_custom_or_with_custom_branch() {
        // OR with a custom subcall branch (non-recursive) + native branch
        let params = Params::default();
        let mut edb = MockEdbView::default();
        let _ = env_logger::builder().is_test(true).try_init();
        // Root has x:7
        let d = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(7))].into(),
        )
        .unwrap();
        let r = d.commitment();
        edb.add_full_dict(d);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // helper(R) = AND(Equal(?R["x"], 7))
        // my_pred(R) = OR(helper(?R), Equal(?R["x"], 8))
        let input = r#"
            helper(R) = AND(
                Equal(?R["x"], 7)
            )

            my_pred(R) = OR(
                helper(?R)
                Equal(?R["x"], 8)
            )

            REQUEST(
                my_pred(?R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run();

        assert_eq!(engine.answers.len(), 1);
        let ans = &engine.answers[0];
        assert_eq!(
            ans.bindings.get(&0).map(|v| v.raw()),
            Some(Value::from(r).raw())
        );
    }

    #[test]
    fn engine_custom_or_rejects_self_recursion() {
        // Bad(R) = OR(Bad(?R), Equal(?R["y"], 1)) should reject the recursive branch and still solve via Equal
        let params = Params::default();
        let mut edb = MockEdbView::default();
        let d = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("y"), Value::from(1))].into(),
        )
        .unwrap();
        let r = d.commitment();
        edb.add_full_dict(d);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let input = r#"
            Bad(R) = OR(
                Bad(?R)
                Equal(?R["y"], 1)
            )

            REQUEST(
                Bad(?R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run();

        assert_eq!(engine.answers.len(), 1);
        let ans = &engine.answers[0];
        assert_eq!(
            ans.bindings.get(&0).map(|v| v.raw()),
            Some(Value::from(r).raw())
        );
        // Registry should record a recursion rejection warning
        assert!(engine
            .rules
            .warnings
            .iter()
            .any(|w| w.contains("self-recursive OR branch")));
    }
}
