use pod2::middleware::{Predicate, Statement, StatementTmpl, StatementTmplArg, Value};
use tracing::{debug, trace};

use crate::{
    custom::{remap_arg, remap_tmpl, CustomRule, RuleRegistry},
    edb::EdbView,
    op::OpRegistry,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, FrameId, PendingCustom, RawOrdValue},
};

#[derive(Clone, Debug)]
pub struct Frame {
    pub id: FrameId,
    /// Goals queued for evaluation: (predicate, template args)
    pub goals: Vec<StatementTmpl>,
    pub store: ConstraintStore,
    pub export: bool,
    pub table_for: Option<CallPattern>,
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
        // Reinsert the suspended goal at the front so it retries on wake
        let mut goals = frame.goals;
        goals.insert(0, goal_stmt);
        let id = frame.id;
        let store = frame.store;
        let export = frame.export;
        let table_for = frame.table_for;
        // Filter out already-bound wildcards
        let on_copy = on.clone();
        let waiting_on: std::collections::HashSet<usize> = on_copy
            .into_iter()
            .filter(|w| !store.bindings.contains_key(w))
            .collect();
        if waiting_on.is_empty() {
            // Nothing to wait on; just re-enqueue
            tracing::debug!(waits = ?on, "re-enqueue without parking");
            self.enqueue(Frame {
                id,
                goals,
                store,
                export,
                table_for,
            });
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
                export,
                table_for,
                waiting_on: waiting_on.clone(),
            },
        );
        tracing::debug!(frame_id = id, waits = ?waiting_on, "parked frame");
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
                        tracing::trace!(frame_id = id, wildcard = wid, "waking parked frame");
                        runnable.push(Frame {
                            id: pf.id,
                            goals: pf.goals,
                            store: pf.store,
                            export: pf.export,
                            table_for: pf.table_for,
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
    export: bool,
    table_for: Option<CallPattern>,
    waiting_on: std::collections::HashSet<usize>,
}

pub struct Engine<'a> {
    pub registry: &'a OpRegistry,
    pub edb: &'a dyn EdbView,
    pub sched: Scheduler,
    pub answers: Vec<crate::types::ConstraintStore>,
    pub rules: RuleRegistry,
    pub policy: SchedulePolicy,
    pub config: EngineConfig,
    steps_executed: u64,
    pub iteration_cap_hit: bool,
    frames_since_epoch: u64,
    tables: std::collections::BTreeMap<CallPattern, Table>,
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
            config: EngineConfig::default(),
            steps_executed: 0,
            iteration_cap_hit: false,
            frames_since_epoch: 0,
            tables: std::collections::BTreeMap::new(),
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

    /// Construct an engine with an explicit configuration.
    pub fn with_config(
        registry: &'a OpRegistry,
        edb: &'a dyn EdbView,
        config: EngineConfig,
    ) -> Self {
        let mut e = Self::new(registry, edb);
        e.config = config;
        e
    }

    /// Update the schedule policy (DFS/BFS).
    pub fn set_schedule(&mut self, policy: SchedulePolicy) {
        self.policy = policy;
    }

    /// Convenience setters for caps.
    pub fn set_iteration_cap(&mut self, cap: Option<u64>) {
        self.config.iteration_cap = cap;
    }
    pub fn set_per_table_fanout_cap(&mut self, cap: Option<u32>) {
        self.config.per_table_fanout_cap = cap;
    }
    pub fn set_per_frame_step_cap(&mut self, cap: Option<u32>) {
        self.config.per_frame_step_cap = cap;
    }
    pub fn set_per_table_epoch_frames(&mut self, frames: Option<u64>) {
        self.config.per_table_epoch_frames = frames;
    }

    /// Convenience: load a parsed Podlang program (custom predicates + request),
    /// register its custom predicates as conjunctive rules, and enqueue the request goals.
    pub fn load_processed(&mut self, processed: &pod2::lang::processor::PodlangOutput) {
        crate::custom::register_rules_from_batch(&mut self.rules, &processed.custom_batch);
        let goals = processed.request.templates().to_vec();
        let id0 = self.sched.new_id();
        self.sched.enqueue(Frame {
            id: id0,
            goals,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
        });
    }

    pub fn run(&mut self) {
        while let Some(frame) = self.sched.dequeue(self.policy) {
            // Global iteration cap
            if let Some(cap) = self.config.iteration_cap {
                if self.steps_executed >= cap {
                    self.iteration_cap_hit = true;
                    debug!(
                        steps = self.steps_executed,
                        cap, "iteration cap hit; aborting run"
                    );
                    break;
                }
            }
            self.steps_executed = self.steps_executed.saturating_add(1);
            // Epoch reset for per-table fanout caps
            if let Some(epoch) = self.config.per_table_epoch_frames {
                self.frames_since_epoch = self.frames_since_epoch.saturating_add(1);
                if self.frames_since_epoch >= epoch {
                    for t in self.tables.values_mut() {
                        t.delivered_this_epoch = 0;
                    }
                    trace!(epoch, "reset per-table fanout epoch counters");
                    self.frames_since_epoch = 0;
                }
            }
            let Frame {
                id,
                goals,
                store,
                export,
                table_for,
            } = frame;
            trace!(frame_id = id, goals = goals.len(), export, "dequeued frame");
            let mut frame_steps: u32 = 0;
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
                // Publish any custom heads to tables before recording the answer
                self.publish_custom_answers(&final_store);
                if export {
                    debug!("exporting completed answer");
                    self.answers.push(final_store);
                }
                if let Some(pat) = table_for.clone() {
                    self.maybe_complete_table(&pat);
                }
                continue;
            }
            // Evaluate goals sequentially; branch on the first goal that yields choices.
            let mut chosen_goal_idx: Option<usize> = None;
            let mut choices_for_goal: Vec<Choice> = Vec::new();
            let mut union_waits: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            let mut any_stmt_for_park: Option<StatementTmpl> = None;
            for (idx, g) in goals.iter().enumerate() {
                // Count this step and yield if exceeding per-frame cap
                frame_steps = frame_steps.saturating_add(1);
                if let Some(cap) = self.config.per_frame_step_cap {
                    if frame_steps > cap {
                        debug!(
                            frame_id = id,
                            cap, "per-frame step cap reached; yielding frame"
                        );
                        self.sched.enqueue(Frame {
                            id,
                            goals: goals.clone(),
                            store: store.clone(),
                            export,
                            table_for: table_for.clone(),
                        });
                        break;
                    }
                }
                let tmpl_args: Vec<StatementTmplArg> = g.args.clone();
                // Handle native vs custom
                let is_custom = matches!(g.pred, Predicate::Custom(_));
                if is_custom {
                    if let Predicate::Custom(ref cpr) = g.pred {
                        let call_args = &goals[idx].args;
                        let pattern = CallPattern::from_call(cpr.clone(), call_args);
                        let is_new = !self.tables.contains_key(&pattern);
                        // Ensure a table exists for this pattern
                        let _ = self
                            .tables
                            .entry(pattern.clone())
                            .or_insert_with(Table::new);
                        if is_new {
                            debug!(predicate = ?cpr, "creating new table and spawning producers");
                            // Spawn producers for each rule: child frames evaluate only the rule body + propagated constraints
                            let rules = self.rules.get(cpr).to_vec();
                            if rules.is_empty() {
                                if let Some(t) = self.tables.get_mut(&pattern) {
                                    t.is_complete = true;
                                }
                                trace!(?pattern, "no rules for predicate; table marked complete");
                            } else {
                                for rule in rules.iter() {
                                    if let Some(mut prod) = self.expand_custom_rule_to_producer(
                                        &goals, &store, idx, cpr, rule,
                                    ) {
                                        trace!("enqueuing rule-body producer");
                                        prod.table_for = Some(pattern.clone());
                                        self.sched.enqueue(prod);
                                    }
                                }
                            }
                        }
                        // Register waiter for the caller and stream any existing table answers, respecting per-table fanout cap
                        trace!(?pattern, "registering waiter for custom call");
                        let waiter = Waiter::from_call(cpr.clone(), idx, &goals, &store, call_args);
                        // Compute deliveries without holding a mutable borrow to self
                        let cap = self.config.per_table_fanout_cap.unwrap_or(u32::MAX);
                        let mut to_deliver: Vec<(Vec<RawOrdValue>, crate::types::OpTag)> =
                            Vec::new();
                        let mut delivered_any = false;
                        if let Some(t) = self.tables.get(&pattern) {
                            let existing: Vec<(Vec<RawOrdValue>, crate::types::OpTag)> = t
                                .answers
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            let mut budget_left = cap.saturating_sub(t.delivered_this_epoch);
                            for (tuple, tag) in existing.into_iter() {
                                if budget_left == 0 {
                                    break;
                                }
                                if waiter.matches(&tuple) {
                                    to_deliver.push((tuple, tag));
                                    budget_left -= 1;
                                    delivered_any = true;
                                }
                            }
                        }
                        // Enqueue continuations
                        for (tuple, tag) in to_deliver.iter() {
                            trace!("stream existing table answer to caller");
                            let cont = waiter.continuation_frame(self, tuple, tag.clone());
                            self.sched.enqueue(cont);
                        }
                        // Update table state and store waiter if needed
                        if let Some(t) = self.tables.get_mut(&pattern) {
                            // increment by the number we actually delivered
                            let inc = to_deliver.len() as u32;
                            if inc > 0 {
                                let cap = self.config.per_table_fanout_cap.unwrap_or(u32::MAX);
                                if t.delivered_this_epoch >= cap {
                                    debug!(
                                        ?pattern,
                                        cap, "per-table fanout cap reached during waiter streaming"
                                    );
                                }
                                t.delivered_this_epoch = t.delivered_this_epoch.saturating_add(inc);
                            }
                            if t.is_complete {
                                trace!(?pattern, "table complete; not storing waiter");
                                if !delivered_any {
                                    debug!(
                                        ?pattern,
                                        "dropping caller: complete table yielded no matches"
                                    );
                                }
                            } else {
                                t.waiters.push(waiter);
                            }
                        }
                        // We handled this goal by tabling; drop this frame (continuations enqueued)
                        chosen_goal_idx = Some(idx);
                        choices_for_goal = Vec::new();
                        break;
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
                    debug!(waits = ?on, "parking frame on wildcards");
                    let stmt_for_park = any_stmt_for_park.unwrap_or_else(|| goals[0].clone());
                    self.sched.park(
                        Frame {
                            id,
                            goals: goals.clone(),
                            store: store.clone(),
                            export,
                            table_for: table_for.clone(),
                        },
                        on,
                        stmt_for_park,
                    );
                    continue;
                } else {
                    // No choices and no suspends → no progress possible; drop frame
                    debug!(frame_id = id, "dropping frame: no choices and no suspends");
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
                            s.push_str(&format!("{i}:"));
                            let raw = v.raw();
                            s.push_str(&format!("{}|", raw.encode_hex::<String>()));
                        }
                        s
                    };
                    let score = match &ch.op_tag {
                        OpTag::Derived { premises } => {
                            if premises
                                .iter()
                                .any(|(_, tag)| matches!(tag, OpTag::GeneratedContains { .. }))
                            {
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
                    let cont = Frame {
                        id: self.sched.new_id(),
                        goals: ng,
                        store: cont_store,
                        export,
                        table_for: table_for.clone(),
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
                let cont = Frame {
                    id,
                    goals: ng,
                    store: cont_store,
                    export,
                    table_for: table_for.clone(),
                };
                self.sched.enqueue(cont);
            }
        }
    }

    // (previous inline expansion path removed; tabling path below is the canonical variant)

    /// Variant of expand_custom_rule that produces a child producer frame which only evaluates
    /// the propagated constraints and the rule body. Continuation of the caller happens via table answers.
    fn expand_custom_rule_to_producer(
        &mut self,
        goals: &[StatementTmpl],
        store: &ConstraintStore,
        goal_idx: usize,
        cpr: &pod2::middleware::CustomPredicateRef,
        rule: &CustomRule,
    ) -> Option<Frame> {
        // Head arity must match call arity
        if rule.head.len() != goals[goal_idx].args.len() {
            return None;
        }
        use std::collections::HashMap;
        let mut map: HashMap<usize, usize> = HashMap::new();
        let mut next_idx = self.next_available_wildcard_index(goals, store) + 1;
        let call_args = &goals[goal_idx].args;
        for (h, call) in rule.head.iter().zip(call_args.iter()) {
            match (h, call) {
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Wildcard(cw)) => {
                    map.insert(hw.index, cw.index);
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::AnchoredKey(cw, _)) => {
                    map.insert(hw.index, cw.index);
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Literal(_v)) => {
                    let target = next_idx;
                    map.insert(hw.index, target);
                    next_idx += 1;
                }
                _ => return None,
            }
        }
        // Ensure all rule-local wildcards (including private ones in the body) are remapped to fresh indices
        for t in rule.body.iter() {
            for a in t.args.iter() {
                match a {
                    StatementTmplArg::Wildcard(w) => {
                        if let std::collections::hash_map::Entry::Vacant(e) = map.entry(w.index) {
                            e.insert(next_idx);
                            next_idx += 1;
                        }
                    }
                    StatementTmplArg::AnchoredKey(w, _) => {
                        if let std::collections::hash_map::Entry::Vacant(e) = map.entry(w.index) {
                            e.insert(next_idx);
                            next_idx += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        let remapped_head: Vec<StatementTmplArg> =
            rule.head.iter().map(|a| remap_arg(a, &map)).collect();
        let remapped_body: Vec<StatementTmpl> =
            rule.body.iter().map(|t| remap_tmpl(t, &map)).collect();

        let mut cont_store = store.clone();
        for (h, call) in remapped_head.iter().zip(call_args.iter()) {
            if let (StatementTmplArg::Wildcard(hw), StatementTmplArg::Literal(v)) = (h, call) {
                cont_store.bindings.insert(hw.index, v.clone());
            }
        }

        use pod2::middleware::NativePredicate;
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
        let mut ng: Vec<StatementTmpl> = Vec::new();
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
            let wcs = crate::prop::wildcards_in_args(&g.args);
            if wcs.iter().all(|w| head_wcs.contains(w)) && !remapped_body.contains(g) {
                ng.push(g.clone());
            }
        }
        ng.extend(remapped_body);

        cont_store.pending_custom.push(PendingCustom {
            rule_id: cpr.clone(),
            head_args: remapped_head,
        });

        Some(Frame {
            id: self.sched.new_id(),
            goals: ng,
            store: cont_store,
            export: false,
            table_for: None,
        })
    }

    fn publish_custom_answers(&mut self, final_store: &crate::types::ConstraintStore) {
        // Scan premises for any CustomDeduction heads and publish them
        for (stmt, tag) in final_store.premises.iter() {
            if let (Statement::Custom(pred, vals), crate::types::OpTag::CustomDeduction { .. }) =
                (stmt, tag)
            {
                let key_vec: Vec<RawOrdValue> = vals.iter().cloned().map(RawOrdValue).collect();
                trace!(predicate = ?pred, tuple = key_vec.len(), "publishing custom head to tables");
                // Publish into all tables matching this predicate whose literal pattern matches the tuple
                let target_patterns: Vec<CallPattern> = self
                    .tables
                    .keys()
                    .filter(|&p| p.pred == *pred && p.matches_tuple(&key_vec))
                    .cloned()
                    .collect();
                for pat in target_patterns.into_iter() {
                    // Compute deliveries without holding mutable borrow during enqueue
                    let mut to_deliver: Vec<Waiter> = Vec::new();
                    let cap = self.config.per_table_fanout_cap.unwrap_or(u32::MAX);
                    let mut exceeded = false;
                    if let Some(entry) = self.tables.get(&pat) {
                        if !entry.answers.contains_key(&key_vec) {
                            // We'll insert and deliver afterwards
                        } else {
                            continue;
                        }
                    }
                    if let Some(entry) = self.tables.get(&pat) {
                        let budget_left = cap.saturating_sub(entry.delivered_this_epoch);
                        if budget_left == 0 {
                            exceeded = true;
                        } else {
                            let mut remaining = budget_left;
                            for w in entry.waiters.iter().cloned() {
                                if remaining == 0 {
                                    break;
                                }
                                if w.matches(&key_vec) {
                                    to_deliver.push(w);
                                    remaining -= 1;
                                }
                            }
                            exceeded = remaining == 0 && cap != u32::MAX;
                        }
                    }
                    // Now mutate: insert answer and update delivered count; enqueue outside of borrow
                    if let Some(entry) = self.tables.get_mut(&pat) {
                        if !entry.answers.contains_key(&key_vec) {
                            entry.answers.insert(key_vec.clone(), tag.clone());
                            debug!(?pat, "inserted new table answer");
                            let inc = to_deliver.len() as u32;
                            if inc > 0 {
                                entry.delivered_this_epoch =
                                    entry.delivered_this_epoch.saturating_add(inc);
                            }
                        }
                    }
                    if exceeded {
                        debug!(?pat, cap, "per-table fanout cap reached during publish");
                    }
                    for w in to_deliver.into_iter() {
                        trace!(?pat, "delivering answer to waiter");
                        let cont = w.continuation_frame(self, &key_vec, tag.clone());
                        self.sched.enqueue(cont);
                    }
                }
            }
        }
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

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    pub iteration_cap: Option<u64>,
    pub per_table_fanout_cap: Option<u32>,
    pub per_frame_step_cap: Option<u32>,
    pub per_table_epoch_frames: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct EngineConfigBuilder {
    cfg: EngineConfig,
}

impl EngineConfigBuilder {
    pub fn new() -> Self {
        Self {
            cfg: EngineConfig::default(),
        }
    }
    pub fn iteration_cap(mut self, cap: u64) -> Self {
        self.cfg.iteration_cap = Some(cap);
        self
    }
    pub fn per_table_fanout_cap(mut self, cap: u32) -> Self {
        self.cfg.per_table_fanout_cap = Some(cap);
        self
    }
    pub fn per_frame_step_cap(mut self, cap: u32) -> Self {
        self.cfg.per_frame_step_cap = Some(cap);
        self
    }
    pub fn per_table_epoch_frames(mut self, frames: u64) -> Self {
        self.cfg.per_table_epoch_frames = Some(frames);
        self
    }
    pub fn build(self) -> EngineConfig {
        self.cfg
    }
}

#[derive(Clone)]
struct Waiter {
    pred: pod2::middleware::CustomPredicateRef,
    goal_idx: usize,
    goals: Vec<StatementTmpl>,
    store: ConstraintStore,
    // For each head position, optional caller wildcard index to bind
    bind_targets: Vec<Option<usize>>,
    // For each head position, optional literal filter that must match
    literal_filters: Vec<Option<Value>>,
}

impl Waiter {
    fn from_call(
        pred: pod2::middleware::CustomPredicateRef,
        goal_idx: usize,
        goals: &[StatementTmpl],
        store: &ConstraintStore,
        call_args: &[StatementTmplArg],
    ) -> Self {
        let mut bind_targets = Vec::with_capacity(call_args.len());
        let mut literal_filters = Vec::with_capacity(call_args.len());
        for a in call_args.iter() {
            match a {
                StatementTmplArg::Wildcard(w) => {
                    bind_targets.push(Some(w.index));
                    literal_filters.push(None);
                }
                StatementTmplArg::Literal(v) => {
                    bind_targets.push(None);
                    literal_filters.push(Some(v.clone()));
                }
                // Heads should not contain AnchoredKeys or None for MVP; treat as non-bindable
                _ => {
                    bind_targets.push(None);
                    literal_filters.push(None);
                }
            }
        }
        Self {
            pred,
            goal_idx,
            goals: goals.to_vec(),
            store: store.clone(),
            bind_targets,
            literal_filters,
        }
    }

    fn matches(&self, tuple: &[RawOrdValue]) -> bool {
        for (i, f) in self.literal_filters.iter().enumerate() {
            if let Some(v) = f {
                if tuple.get(i).map(|rv| rv.0.raw()) != Some(v.raw()) {
                    return false;
                }
            }
        }
        true
    }

    fn continuation_frame(
        &self,
        engine: &mut Engine,
        tuple: &[RawOrdValue],
        head_tag: crate::types::OpTag,
    ) -> Frame {
        let mut cont_store = self.store.clone();
        // Apply head bindings to caller store
        for (i, maybe_idx) in self.bind_targets.iter().enumerate() {
            if let Some(idx) = maybe_idx {
                if let Some(rv) = tuple.get(i) {
                    cont_store.bindings.insert(*idx, rv.0.clone());
                }
            }
        }
        // Append the head proof step (CustomDeduction) as a premise for provenance
        let head_stmt = Statement::Custom(
            self.pred.clone(),
            tuple.iter().map(|rv| rv.0.clone()).collect(),
        );
        cont_store.premises.push((head_stmt, head_tag));

        let mut ng = self.goals.clone();
        // Remove the custom goal at goal_idx
        if self.goal_idx < ng.len() {
            ng.remove(self.goal_idx);
        }
        Frame {
            id: engine.sched.new_id(),
            goals: ng,
            store: cont_store,
            export: true,
            table_for: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallPattern {
    pred: pod2::middleware::CustomPredicateRef,
    // For each head position, Some(literal) or None (variable/AK)
    literals: Vec<Option<RawOrdValue>>,
}

impl CallPattern {
    fn from_call(pred: pod2::middleware::CustomPredicateRef, args: &[StatementTmplArg]) -> Self {
        let mut lits = Vec::with_capacity(args.len());
        for a in args.iter() {
            match a {
                StatementTmplArg::Literal(v) => lits.push(Some(RawOrdValue(v.clone()))),
                _ => lits.push(None),
            }
        }
        Self {
            pred,
            literals: lits,
        }
    }
    fn matches_tuple(&self, tuple: &[RawOrdValue]) -> bool {
        for (i, maybe) in self.literals.iter().enumerate() {
            if let Some(rv) = maybe {
                if tuple.get(i) != Some(rv) {
                    return false;
                }
            }
        }
        true
    }
}

impl std::cmp::PartialOrd for CallPattern {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::cmp::Ord for CallPattern {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Order by predicate debug string, then by literals vector
        let a = format!("{:?}", self.pred);
        let b = format!("{:?}", other.pred);
        match a.cmp(&b) {
            std::cmp::Ordering::Equal => self.literals.cmp(&other.literals),
            o => o,
        }
    }
}

struct Table {
    // Deterministic map: head tuple -> proof tag for the head
    answers: std::collections::BTreeMap<Vec<RawOrdValue>, crate::types::OpTag>,
    waiters: Vec<Waiter>,
    is_complete: bool,
    delivered_this_epoch: u32,
}

impl Table {
    fn new() -> Self {
        Self {
            answers: std::collections::BTreeMap::new(),
            waiters: Vec::new(),
            is_complete: false,
            delivered_this_epoch: 0,
        }
    }
}

impl<'a> Engine<'a> {
    fn maybe_complete_table(&mut self, pat: &CallPattern) {
        // If there are no runnable or parked frames producing for this pattern, mark complete and prune waiters
        let has_runnable = self
            .sched
            .runnable
            .iter()
            .any(|f| matches!(f, Frame { table_for: Some(p), .. } if p == pat));
        let has_parked = self
            .sched
            .parked
            .values()
            .any(|pf| matches!(pf, ParkedFrame { table_for: Some(p), .. } if p == pat));
        if !has_runnable && !has_parked {
            if let Some(t) = self.tables.get_mut(pat) {
                t.is_complete = true;
                t.waiters.clear();
                debug!(?pat, "table marked complete and waiters pruned");
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
    use tracing_subscriber::{fmt, EnvFilter};

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
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
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
        engine.sched.enqueue(Frame {
            id: id0,
            goals,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
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
    fn engine_iteration_cap_aborts_run() {
        // Simple request that would normally produce at least one answer
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("k"), Value::from(1))].into(),
        )
        .unwrap();
        let mut edb = MockEdbView::default();
        edb.add_full_dict(dict);

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
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        // Set a very small iteration cap to force early abort
        engine.config.iteration_cap = Some(0);
        engine.run();
        assert!(engine.iteration_cap_hit, "expected iteration cap to be hit");
        // May or may not have answers depending on timing; just assert no panic and flag set
    }

    #[test]
    fn engine_fair_delivery_interleaves_with_independent_goal() {
        // Many roots for k:1 to create a large table of answers, and a separate small goal Equal(?S["x"],3).
        let params = Params::default();
        let mut edb = MockEdbView::default();
        // Add 20 distinct roots with k:1 (make roots unique by adding a varying filler key)
        for i in 0..20 {
            let d = Dictionary::new(
                params.max_depth_mt_containers,
                [
                    (Key::from("k"), Value::from(1)),
                    (Key::from("__i"), Value::from(i)),
                ]
                .into(),
            )
            .unwrap();
            edb.add_full_dict(d);
        }
        // Add independent root S with x:3
        let d_s = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(3))].into(),
        )
        .unwrap();
        let root_s = d_s.commitment();
        edb.add_full_dict(d_s);

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // Custom predicate enumerates all roots with k:1 via entries
        let program = r#"
            make_r(R) = AND(
                Equal(?R["k"], 1)
            )

            REQUEST(
                make_r(?R)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        // Also enqueue an independent goal Equal(?S["x"], 3)
        let processed2 = parse(
            r#"REQUEST(
                Equal(?S["x"], 3)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let goals2 = processed2.request.templates().to_vec();
        let id2 = engine.sched.new_id();
        engine.sched.enqueue(Frame {
            id: id2,
            goals: goals2,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
        });

        // Configure caps to allow only 1 table delivery per epoch and reset every frame
        engine.policy = SchedulePolicy::BreadthFirst;
        engine.config.per_table_fanout_cap = Some(1);
        engine.config.per_table_epoch_frames = Some(1);
        engine.config.per_frame_step_cap = Some(1);

        engine.run();

        // Verify that the independent goal completed: look for Equal(AK(root_s, "x"), 3) in premises
        use pod2::middleware::{AnchoredKey, Statement, ValueRef};
        let mut saw_equal_s = false;
        for st in engine.answers.iter() {
            for (stmt, _) in st.premises.iter() {
                if let Statement::Equal(
                    ValueRef::Key(AnchoredKey { root, key }),
                    ValueRef::Literal(v),
                ) = stmt
                {
                    if *root == root_s && key.name() == "x" && *v == Value::from(3) {
                        saw_equal_s = true;
                    }
                }
            }
        }
        assert!(
            saw_equal_s,
            "independent Equal(?S[\"x\"],3) should complete under fanout caps"
        );
    }

    #[test]
    fn scheduler_policy_depth_first_vs_breadth_first() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
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
        eng_dfs.sched.enqueue(Frame {
            id: id_a,
            goals: vec![],
            store: s1,
            export: true,
            table_for: None,
        });
        let id_b = eng_dfs.sched.new_id();
        eng_dfs.sched.enqueue(Frame {
            id: id_b,
            goals: vec![],
            store: s2,
            export: true,
            table_for: None,
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
        eng_bfs.sched.enqueue(Frame {
            id: id_c,
            goals: vec![],
            store: t1,
            export: true,
            table_for: None,
        });
        let id_d = eng_bfs.sched.new_id();
        eng_bfs.sched.enqueue(Frame {
            id: id_d,
            goals: vec![],
            store: t2,
            export: true,
            table_for: None,
        });
        eng_bfs.run();
        assert_eq!(eng_bfs.answers.len(), 2);
        assert_eq!(eng_bfs.answers[0].bindings.get(&0), Some(&Value::from(1)));
        assert_eq!(eng_bfs.answers[1].bindings.get(&0), Some(&Value::from(2)));
    }

    #[test]
    fn determinism_golden_many_choices() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
        // Build 5 roots each with k:1; query Equal(?R["k"], 1). Ordering should be stable across runs.
        let params = Params::default();
        let mut edb = MockEdbView::default();
        let mut roots = Vec::new();
        for _i in 0..5 {
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
        engine1.sched.enqueue(Frame {
            id: id1,
            goals: goals.clone(),
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
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
        engine2.sched.enqueue(Frame {
            id: id2,
            goals,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
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
        // Expand the custom rule (producer variant used by tabling)
        let frame = engine
            .expand_custom_rule_to_producer(
                &parent_goals,
                &ConstraintStore::default(),
                1,
                &cpr,
                &rules[0],
            )
            .expect("frame");
        // The first goal should be the body Equal, not the unrelated Lt(?Z,5)
        let Frame { goals, .. } = frame;
        // The propagated list should not include Lt(?Z,5) since Z is not in helper head
        use pod2::middleware::NativePredicate;
        if let Predicate::Native(NativePredicate::Lt) = goals[0].pred {
            panic!("unexpected propagation of private Lt");
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
        engine.sched.enqueue(Frame {
            id: id0,
            goals,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
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
        engine.sched.enqueue(Frame {
            id: id0,
            goals,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
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
        engine.sched.enqueue(Frame {
            id: id0,
            goals,
            store: ConstraintStore::default(),
            export: true,
            table_for: None,
        });
        engine.run();

        assert!(!engine.answers.is_empty());
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

        assert!(!engine.answers.is_empty());
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

        assert!(!engine.answers.is_empty());
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

        assert!(!engine.answers.is_empty());
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

    #[test]
    fn engine_custom_and_self_recursion_yields_empty_rule_table_completed() {
        // AND body with a self-recursive statement is rejected at registration → zero rules for that predicate.
        // The table should be marked complete immediately and no waiter is stored.
        let edb = MockEdbView::default();
        let reg = OpRegistry::default();

        // Define a self-recursive AND predicate and call it.
        let program = r#"
            bad(A) = AND(
                bad(?A)
            )

            REQUEST(
                bad(1)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        // Register rules (self-recursive AND is rejected → no rules for 'bad') and enqueue request
        engine.load_processed(&processed);
        engine.run();

        // Expect no answers
        assert!(engine.answers.is_empty());
        // Expect one table, marked complete, with no waiters and no answers
        assert_eq!(engine.tables.len(), 1, "expected one table for bad/1");
        let (_pat, tbl) = engine.tables.iter().next().unwrap();
        assert!(tbl.is_complete, "table should be marked complete");
        assert!(tbl.waiters.is_empty(), "no waiters should be stored");
        assert!(tbl.answers.is_empty(), "no answers should exist");
    }

    #[test]
    fn engine_recursion_mutual_via_tabling_nat_down() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
        // Define NAT recursion using mutual recursion with a decrement defined via SumOf:
        // dec(A,B) :- SumOf(B, 1, A)
        // step(N)  :- dec(N, M), nat_down(M)
        // nat_down(N) :- OR(Equal(N,0), step(N))
        let edb = MockEdbView::default();
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_sumof_handlers(&mut reg);

        let program = r#"
            dec(A, B) = AND(
                SumOf(?A, ?B, 1)
            )

            step(N, private: M) = AND(
                dec(?N, ?M)
                nat_down(?M)
            )

            nat_down(N) = OR(
                Equal(?N, 0)
                step(?N)
            )

            REQUEST(
                nat_down(3)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run();

        // Expect at least one answer and that a CustomDeduction head nat_down(3) appears in premises
        assert!(!engine.answers.is_empty());
        use pod2::middleware::Statement;
        let mut saw_nat3 = false;
        for st in engine.answers.iter() {
            for (stmt, tag) in st.premises.iter() {
                if let Statement::Custom(_, vals) = stmt {
                    // Identify nat_down by its name in CustomPredicateRef debug (best-effort)
                    if vals.len() == 1
                        && *vals.first().unwrap() == Value::from(3)
                        && matches!(tag, crate::types::OpTag::CustomDeduction { .. })
                    {
                        saw_nat3 = true;
                    }
                }
            }
        }
        assert!(saw_nat3, "expected nat_down(3) CustomDeduction in premises");
    }

    #[test]
    fn engine_mutual_recursion_even_odd_via_dec() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
        // Mutual recursion with base case even(0)
        let edb = MockEdbView::default();
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_sumof_handlers(&mut reg);

        let program = r#"
            dec(A, B) = AND(
                SumOf(?A, ?B, 1)
            )

            even_step(N, private: M) = AND(
                dec(?N, ?M)
                odd(?M)
            )

            even(N) = OR(
                Equal(?N, 0)
                even_step(?N)
            )

            odd(N, private: M) = AND(
                dec(?N, ?M)
                even(?M)
            )

            REQUEST(
                even(4)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run();

        assert!(
            !engine.answers.is_empty(),
            "expected at least one answer proving even(4)"
        );
    }
}
