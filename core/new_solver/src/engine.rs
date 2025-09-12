use std::time::{Duration, Instant};

use pod2::middleware::{Predicate, Statement, StatementTmpl, StatementTmplArg, Value};
use thiserror::Error;
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

#[derive(Debug, Error, Clone)]
pub enum EngineError {
    #[error("No OpHandlers registered for native predicate {predicate:?}. Did you forget to register its handlers?")]
    MissingHandlers {
        predicate: pod2::middleware::NativePredicate,
    },
    #[error("Iteration cap hit after {steps} steps")]
    IterationCap { steps: u64 },
    #[error("Wall-clock timeout after {elapsed_ms} ms")]
    Timeout { elapsed_ms: u128 },
    #[error("No answers found")]
    NoAnswers,
}

#[derive(Default)]
pub struct Scheduler {
    pub runnable: std::collections::VecDeque<Frame>,
    next_id: FrameId,
    // Suspension bookkeeping
    waitlist: std::collections::BTreeMap<usize, std::collections::BTreeSet<FrameId>>,
    parked: std::collections::HashMap<FrameId, ParkedFrame>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FinalizeAction {
    Continue,
    EarlyExit,
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

    pub fn park(&mut self, frame: Frame, on: Vec<usize>, _goal_stmt: StatementTmpl) {
        // Reinsert the suspended goal at the front so it retries on wake
        let Frame {
            id,
            goals,
            store,
            export,
            table_for,
        } = frame;
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
    // Branch-and-bound: best (lowest) operation count observed for any exported answer
    best_ops_so_far: Option<usize>,
    // Branch-and-bound: best (lowest) unique input pods observed for any exported answer
    best_inputs_so_far: Option<usize>,
    /// Last fatal error encountered during run.
    pub last_error: Option<EngineError>,
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
            best_ops_so_far: None,
            best_inputs_so_far: None,
            last_error: None,
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

    pub fn run(&mut self) -> Result<(), EngineError> {
        let start = Instant::now();
        while let Some(frame) = self.sched.dequeue(self.policy) {
            // Bounds: iteration and wall-clock
            self.check_iteration_and_timeout(start)?;
            self.steps_executed = self.steps_executed.saturating_add(1);
            // Epoch reset for per-table fanout caps
            self.maybe_reset_epoch_counters();
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
                match self.finalize_frame(id, store, export, table_for)? {
                    FinalizeAction::Continue => continue,
                    FinalizeAction::EarlyExit => return Ok(()),
                }
            }
            // Evaluate goals sequentially; branch on the first goal that yields choices.
            let mut chosen_goal_idx: Option<usize> = None;
            let mut choices_for_goal: Vec<Choice> = Vec::new();
            let mut union_waits: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            let mut any_stmt_for_park: Option<StatementTmpl> = None;
            let mut frame_contradiction = false;
            for (idx, g) in goals.iter().enumerate() {
                // Count this step and yield if exceeding per-frame cap
                frame_steps = frame_steps.saturating_add(1);
                if self.should_yield_frame(frame_steps) {
                    self.sched.enqueue(Frame {
                        id,
                        goals: goals.clone(),
                        store: store.clone(),
                        export,
                        table_for: table_for.clone(),
                    });
                    break;
                }
                if matches!(g.pred, Predicate::Custom(_))
                    && self.handle_custom_goal(idx, &goals, &store)
                {
                    chosen_goal_idx = Some(idx);
                    // Do not clear choices here; tabling is a valid continuation
                    break;
                }
                if let Predicate::Native(p) = g.pred {
                    let choices = self.handle_native_goal(
                        p,
                        &g.args,
                        g,
                        &store,
                        &mut union_waits,
                        &mut any_stmt_for_park,
                    )?;
                    if !choices.is_empty() {
                        chosen_goal_idx = Some(idx);
                        choices_for_goal = choices;
                        break;
                    } else if union_waits.is_empty() {
                        // No choices and no new suspensions means this goal is a contradiction
                        frame_contradiction = true;
                        break;
                    }
                }
            }

            if frame_contradiction {
                debug!(frame_id = id, "dropping frame: native goal contradiction");
                continue;
            }

            if let Some(i) = chosen_goal_idx {
                if !choices_for_goal.is_empty() {
                    let best = self.dedup_and_score_choices(choices_for_goal);
                    self.enqueue_continuations_for_choices(
                        best,
                        i,
                        &goals,
                        &store,
                        export,
                        table_for.clone(),
                    );
                }
                // If a custom goal was chosen, even with no immediate choices,
                // we've made progress via tabling. Continue to next frame.
                continue;
            }

            // No goal was chosen to produce choices. If any goal suspended, park.
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
            } else {
                // No choices and no suspends → no progress possible; drop frame
                debug!(frame_id = id, "dropping frame: no choices and no suspends");
            }
        }
        if self.answers.is_empty() {
            return Err(EngineError::NoAnswers);
        }
        Ok(())
    }

    #[inline]
    fn check_iteration_and_timeout(&mut self, start: Instant) -> Result<(), EngineError> {
        if let Some(cap) = self.config.iteration_cap {
            if self.steps_executed >= cap {
                self.iteration_cap_hit = true;
                debug!(
                    steps = self.steps_executed,
                    cap, "iteration cap hit; aborting run"
                );
                return Err(EngineError::IterationCap {
                    steps: self.steps_executed,
                });
            }
        }
        if let Some(timeout) = self.config.wall_clock_timeout {
            if start.elapsed() >= timeout {
                let ms = start.elapsed().as_millis();
                debug!(ms, "wall-clock timeout; aborting run");
                return Err(EngineError::Timeout { elapsed_ms: ms });
            }
        }
        Ok(())
    }

    #[inline]
    fn maybe_reset_epoch_counters(&mut self) {
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
    }

    #[inline]
    fn should_yield_frame(&self, frame_steps: u32) -> bool {
        if let Some(cap) = self.config.per_frame_step_cap {
            if frame_steps > cap {
                debug!(cap, "per-frame step cap reached; yielding frame");
                return true;
            }
        }
        false
    }

    fn finalize_frame(
        &mut self,
        id: FrameId,
        mut store: ConstraintStore,
        export: bool,
        table_for: Option<CallPattern>,
    ) -> Result<FinalizeAction, EngineError> {
        // Record a completed answer (bindings and any accumulated premises)
        let t_final_start = std::time::Instant::now();
        // Materialize any pending custom deductions as head proof steps
        if !store.pending_custom.is_empty() {
            let t_mat_start = std::time::Instant::now();
            let mut pendings = std::mem::take(&mut store.pending_custom);
            // Process innermost first to compress bodies into heads iteratively
            if let Some(p) = pendings.pop() {
                if let Some(head) =
                    crate::util::instantiate_custom(&p.rule_id, &p.head_args, &store.bindings)
                {
                    // Take the body premises added after this pending head was registered
                    let body = store.premises.split_off(p.base_premises_len);
                    store.premises.push((
                        head,
                        crate::types::OpTag::CustomDeduction {
                            rule_id: p.rule_id.clone(),
                            premises: body,
                        },
                    ));
                }
                // Restore any outer pending frames
                store.pending_custom = pendings;
            }
            log_if_slow(
                "finalize: materialize pending custom heads",
                t_mat_start,
                50,
            );
        }
        // Publish any custom heads to tables before recording the answer
        let t_pub_start = std::time::Instant::now();
        let early = self.publish_custom_answers(&store);
        log_if_slow("finalize: publish_custom_answers", t_pub_start, 50);
        if early {
            return Ok(FinalizeAction::EarlyExit);
        }
        if export {
            // Recompute accurate operation count for pruning/metrics from full proof DAG
            let (ops, _inputs) = crate::util::proof_cost(&store);
            store.operation_count = ops;
            debug!("exporting completed answer");
            self.answers.push(store);
            // Update best bound on operations for branch-and-bound
            let (ops, inputs) = self
                .answers
                .last()
                .map(|st| (st.operation_count, crate::util::proof_cost(st).1))
                .unwrap_or((0, 0));
            self.best_ops_so_far = Some(self.best_ops_so_far.map_or(ops, |b| b.min(ops)));
            self.best_inputs_so_far =
                Some(self.best_inputs_so_far.map_or(inputs, |b| b.min(inputs)));
            // Early exit mode: return immediately after the first exported answer
            if self.config.early_exit_on_first_answer {
                return Ok(FinalizeAction::EarlyExit);
            }
        } else {
            // Not exported: still retain store for table publishing above
        }
        if let Some(pat) = table_for.clone() {
            self.maybe_complete_table(&pat);
        }
        let dt_total = t_final_start.elapsed();
        if dt_total.as_millis() > 100 {
            trace!(
                frame_id = id,
                ms = dt_total.as_millis(),
                "finalize: total finalize time"
            );
        }
        Ok(FinalizeAction::Continue)
    }

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
        let mut head_bindings = store.bindings.clone();

        for (h, call) in rule.head.iter().zip(call_args.iter()) {
            match (h, call) {
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Wildcard(cw)) => {
                    map.insert(hw.index, cw.index);
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::AnchoredKey(cw, _)) => {
                    map.insert(hw.index, cw.index);
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Literal(v)) => {
                    let target = next_idx;
                    map.insert(hw.index, target);
                    head_bindings.insert(target, v.clone());
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
        cont_store.bindings = head_bindings;
        // Accumulate structural lower bound for this rule's body
        cont_store.accumulated_lb_ops = cont_store
            .accumulated_lb_ops
            .saturating_add(rule.min_native_cost + rule.min_subcall_count);
        // Pre-bind remapped head wildcards from caller args that are literals or bound wildcards
        for (h, call) in remapped_head.iter().zip(call_args.iter()) {
            match (h, call) {
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Literal(v)) => {
                    cont_store.bindings.insert(hw.index, v.clone());
                }
                (StatementTmplArg::Wildcard(hw), StatementTmplArg::Wildcard(cw)) => {
                    if let Some(v) = store.bindings.get(&cw.index) {
                        cont_store.bindings.insert(hw.index, v.clone());
                    }
                }
                _ => {}
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
        // Reorder body: native first, then non-self custom, then self-recursive custom
        let mut natives: Vec<StatementTmpl> = Vec::new();
        let mut custom_other: Vec<StatementTmpl> = Vec::new();
        let mut custom_self: Vec<StatementTmpl> = Vec::new();
        for t in remapped_body.into_iter() {
            match t.pred {
                Predicate::Native(_) => natives.push(t),
                Predicate::Custom(ref r) if *r == *cpr => custom_self.push(t),
                Predicate::Custom(_) => custom_other.push(t),
                _ => custom_other.push(t),
            }
        }
        let mut ordered_body: Vec<StatementTmpl> = Vec::new();
        ordered_body.extend(natives);
        ordered_body.extend(custom_other);
        ordered_body.extend(custom_self);
        ng.extend(ordered_body.clone());

        cont_store.pending_custom.push(PendingCustom {
            rule_id: cpr.clone(),
            head_args: remapped_head,
            base_premises_len: cont_store.premises.len(),
        });
        trace!(pred = ?crate::debug::CustomPredicateRefDebug(cpr.clone()), goals = ?ng, "spawned producer goals");
        Some(Frame {
            id: self.sched.new_id(),
            goals: ng,
            store: cont_store,
            export: false,
            table_for: None,
        })
    }

    #[inline]
    fn instantiate_call_args(
        &self,
        store: &ConstraintStore,
        call_args: &[StatementTmplArg],
    ) -> Vec<StatementTmplArg> {
        call_args
            .iter()
            .cloned()
            .map(|a| match a {
                StatementTmplArg::Wildcard(w) => store
                    .bindings
                    .get(&w.index)
                    .cloned()
                    .map(StatementTmplArg::Literal)
                    .unwrap_or(StatementTmplArg::Wildcard(w)),
                other => other,
            })
            .collect()
    }

    // Returns true if the custom goal was handled (either pruned or tabled), indicating the caller should break.
    fn handle_custom_goal(
        &mut self,
        idx: usize,
        goals: &[StatementTmpl],
        store: &ConstraintStore,
    ) -> bool {
        let g = &goals[idx];
        let Predicate::Custom(ref cpr) = g.pred else {
            return false;
        };
        let inst_call_args = self.instantiate_call_args(store, &goals[idx].args);
        let pattern = CallPattern::from_call(cpr.clone(), &inst_call_args);
        // Enforce head arguments policy: only literals or wildcards are allowed
        let head_args_ok = inst_call_args.iter().all(|a| {
            matches!(
                a,
                StatementTmplArg::Literal(_) | StatementTmplArg::Wildcard(_)
            )
        });
        if !head_args_ok {
            debug!(
                ?pattern,
                "rejecting custom call: head args must be literals or wildcards"
            );
            return true;
        }
        if self.config.branch_and_bound_on_ops
            && self.custom_call_exceeds_bound(cpr, goals, idx, store)
        {
            debug!(?pattern, "pruning custom call: LB exceeds best bound");
            return true;
        }
        let is_new = !self.tables.contains_key(&pattern);
        let entry = self
            .tables
            .entry(pattern.clone())
            .or_insert_with(Table::new);
        // Seed table with any EDB-provided custom matches (CopyStatement proofs)
        let filters: Vec<Option<Value>> = inst_call_args
            .iter()
            .map(|a| match a {
                StatementTmplArg::Literal(v) => Some(v.clone()),
                StatementTmplArg::Wildcard(_) => None,
                _ => None,
            })
            .collect();
        let matches = self.edb.custom_matches(cpr, &filters);
        if !matches.is_empty() {
            for (args, src) in matches.into_iter() {
                let key_vec: Vec<RawOrdValue> = args.iter().cloned().map(RawOrdValue).collect();
                let tags = entry.answers.entry(key_vec).or_default();
                let tag = crate::types::OpTag::CopyStatement { source: src };
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
        if is_new {
            debug!(predicate = ?crate::debug::CustomPredicateRefDebug(cpr.clone()), "creating new table and spawning producers");
            let rules = self.rules.get(cpr).to_vec();
            if rules.is_empty() {
                if let Some(t) = self.tables.get_mut(&pattern) {
                    t.is_complete = true;
                }
                trace!(?pattern, "no rules for predicate; table marked complete");
            } else {
                for rule in rules.iter() {
                    if let Some(mut prod) =
                        self.expand_custom_rule_to_producer(goals, store, idx, cpr, rule)
                    {
                        trace!("enqueuing rule-body producer");
                        prod.table_for = Some(pattern.clone());
                        self.sched.enqueue(prod);
                    }
                }
            }
        }
        trace!(?pattern, "registering waiter for custom call");
        let waiter = Waiter::from_call(cpr.clone(), idx, goals, store, &inst_call_args);
        let cap = self.config.per_table_fanout_cap.unwrap_or(u32::MAX);
        let mut to_deliver: Vec<(Vec<RawOrdValue>, crate::types::OpTag)> = Vec::new();
        let mut delivered_any = false;
        if let Some(t) = self.tables.get(&pattern) {
            let (sel, _inc, exceeded) =
                select_answers_for_waiter(t, &waiter, cap, t.delivered_this_epoch);
            to_deliver = sel;
            delivered_any = !to_deliver.is_empty();
            if exceeded {
                debug!(
                    ?pattern,
                    cap, "per-table fanout cap reached during waiter streaming"
                );
            }
        }
        for (tuple, tag) in to_deliver.iter() {
            trace!("stream existing table answer to caller");
            let cont = waiter.continuation_frame(self, tuple, tag.clone());
            self.sched.enqueue(cont);
        }
        if let Some(t) = self.tables.get_mut(&pattern) {
            let inc = to_deliver.len() as u32;
            if inc > 0 {
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
                let is_dup = t.waiters.iter().any(|w| w.same_signature(&waiter));
                if is_dup {
                    trace!(?pattern, "duplicate waiter ignored");
                } else {
                    t.waiters.push(waiter);
                }
            }
        }
        // handled (tabled); caller should break
        true
    }

    fn handle_native_goal(
        &mut self,
        goal_pred: pod2::middleware::NativePredicate,
        tmpl_args: &[StatementTmplArg],
        g: &StatementTmpl,
        store: &ConstraintStore,
        union_waits: &mut std::collections::HashSet<usize>,
        any_stmt_for_park: &mut Option<StatementTmpl>,
    ) -> Result<Vec<Choice>, EngineError> {
        trace!(pred = ?goal_pred, args = ?tmpl_args, "processing native goal");
        let handlers = self.registry.get(goal_pred);
        if handlers.is_empty() {
            debug!(
                ?goal_pred,
                "no handlers registered for native predicate; aborting run"
            );
            return Err(EngineError::MissingHandlers {
                predicate: goal_pred,
            });
        }
        let mut local_choices: Vec<Choice> = Vec::new();
        for h in handlers {
            match h.propagate(tmpl_args, &mut store.clone(), self.edb) {
                PropagatorResult::Entailed { bindings, op_tag } => {
                    local_choices.push(Choice { bindings, op_tag })
                }
                PropagatorResult::Choices { mut alternatives } => {
                    local_choices.append(&mut alternatives)
                }
                PropagatorResult::Suspend { on } => {
                    if any_stmt_for_park.is_none() {
                        *any_stmt_for_park = Some(g.clone());
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
        trace!(pred = ?goal_pred, choices = local_choices.len(), waits = ?union_waits, "native goal outcome");
        Ok(local_choices)
    }

    fn publish_custom_answers(&mut self, final_store: &crate::types::ConstraintStore) -> bool {
        let t_start = std::time::Instant::now();
        let mut heads_scanned = 0usize;
        let mut answers_inserted = 0usize;
        let mut deliveries = 0usize;
        let mut early_exit_triggered = false;
        // Scan premises for any CustomDeduction heads and publish them
        for (stmt, tag) in final_store.premises.iter() {
            if let (Statement::Custom(pred, vals), crate::types::OpTag::CustomDeduction { .. }) =
                (stmt, tag)
            {
                heads_scanned += 1;
                let key_vec: Vec<RawOrdValue> = vals.iter().cloned().map(RawOrdValue).collect();
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
                        let (sel, _inc, exc) = select_waiters_for_answer(
                            entry,
                            &key_vec,
                            cap,
                            entry.delivered_this_epoch,
                        );
                        to_deliver = sel;
                        exceeded = exc;
                    }
                    // Now mutate: insert answer tag and update delivered count; enqueue outside of borrow
                    let mut inserted_new_tag = false;
                    if let Some(entry) = self.tables.get_mut(&pat) {
                        let tags = entry.answers.entry(key_vec.clone()).or_default();
                        if !tags.contains(tag) {
                            tags.push(tag.clone());
                            inserted_new_tag = true;
                            debug!(?pat, "inserted/extended table answer with new proof tag");
                            answers_inserted += 1;
                        }
                    }
                    if exceeded {
                        debug!(?pat, cap, "per-table fanout cap reached during publish");
                    }
                    if inserted_new_tag {
                        // Deliver only if this tag is new for this tuple
                        if let Some(entry) = self.tables.get_mut(&pat) {
                            let inc = to_deliver.len() as u32;
                            if inc > 0 {
                                entry.delivered_this_epoch =
                                    entry.delivered_this_epoch.saturating_add(inc);
                            }
                        }
                        for w in to_deliver.into_iter() {
                            trace!(?pat, "delivering answer to waiter");
                            let cont = w.continuation_frame(self, &key_vec, tag.clone());
                            if self.config.early_exit_on_first_answer
                                && cont.export
                                && cont.goals.is_empty()
                            {
                                let mut store = cont.store.clone();
                                let (ops, _inputs) = crate::util::proof_cost(&store);
                                store.operation_count = ops;
                                self.answers.push(store);
                                let (ops, inputs) = self
                                    .answers
                                    .last()
                                    .map(|st| (st.operation_count, crate::util::proof_cost(st).1))
                                    .unwrap_or((0, 0));
                                self.best_ops_so_far =
                                    Some(self.best_ops_so_far.map_or(ops, |b| b.min(ops)));
                                self.best_inputs_so_far =
                                    Some(self.best_inputs_so_far.map_or(inputs, |b| b.min(inputs)));
                                early_exit_triggered = true;
                                break;
                            } else {
                                self.sched.enqueue(cont);
                                deliveries += 1;
                            }
                        }
                    }
                    if early_exit_triggered {
                        break;
                    }
                }
            }
            if early_exit_triggered {
                break;
            }
        }
        let dt = t_start.elapsed();
        if dt.as_millis() > 50 {
            trace!(
                ms = dt.as_millis(),
                heads_scanned,
                answers_inserted,
                deliveries,
                "publish_custom_answers: timing"
            );
        }
        early_exit_triggered
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

    #[inline]
    fn dedup_and_score_choices(&self, choices: Vec<Choice>) -> Vec<Choice> {
        use std::collections::BTreeMap;

        use crate::types::OpTag;
        // Stable map keyed by a canonical string of bindings
        let mut best: BTreeMap<String, (i32, Choice)> = BTreeMap::new();
        for ch in choices.into_iter() {
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
        best.into_iter().map(|(_, (_, ch))| ch).collect()
    }

    fn enqueue_continuations_for_choices(
        &mut self,
        choices: Vec<Choice>,
        chosen_goal_idx: usize,
        goals: &[StatementTmpl],
        store: &ConstraintStore,
        export: bool,
        table_for: Option<CallPattern>,
    ) {
        for ch in choices.into_iter() {
            let mut cont_store = store.clone();
            for (w, v) in ch.bindings.iter().cloned() {
                cont_store.bindings.insert(w, v);
            }
            // Wake any parked frames that were waiting on these bindings
            for woke in self.sched.wake_with_bindings(&ch.bindings) {
                self.sched.enqueue(woke);
            }
            let mut ng = goals.to_vec();
            ng.remove(chosen_goal_idx);
            // Record head proof step for this goal in the continuation store
            let head_tmpl = &goals[chosen_goal_idx];
            if let Some(head) = crate::util::instantiate_goal(head_tmpl, &cont_store.bindings) {
                // Record proof step and update optimization counters
                let tag = ch.op_tag.clone();
                record_head_step(&mut cont_store, head, tag.clone());
            }
            // Branch-and-bound pruning by POD budget using ops/inputs limits
            if self.config.branch_and_bound_on_ops {
                let (realized_ops, realized_inputs) = crate::util::proof_cost(&cont_store);
                let remaining_native = ng
                    .iter()
                    .filter(|t| matches!(t.pred, Predicate::Native(_)))
                    .count();
                let remaining_custom = ng
                    .iter()
                    .filter(|t| matches!(t.pred, Predicate::Custom(_)))
                    .count();
                let exceeds = exceeds_best_pods_bound(
                    &self.config,
                    self.best_ops_so_far,
                    self.best_inputs_so_far,
                    realized_ops,
                    realized_inputs,
                    remaining_native,
                    remaining_custom,
                    0,
                );
                if exceeds {
                    continue;
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
    }

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

    #[inline]
    fn custom_call_exceeds_bound(
        &self,
        cpr: &pod2::middleware::CustomPredicateRef,
        goals: &[StatementTmpl],
        current_idx: usize,
        store: &ConstraintStore,
    ) -> bool {
        let (realized_ops, realized_inputs) = crate::util::proof_cost(store);
        // Structural LB: minimal body cost across rules (native steps + subcalls)
        let rules = self.rules.get(cpr);
        let mut min_body_cost = if rules.is_empty() {
            1usize
        } else {
            rules
                .iter()
                .map(|r| r.min_native_cost + r.min_subcall_count)
                .min()
                .unwrap_or(1)
        };
        // Consider an EDB copy path: if any existing custom head matches the literal mask, LB can be 1 op
        let call_args = &goals[current_idx].args;
        let filters: Vec<Option<Value>> = call_args
            .iter()
            .map(|a| match a {
                StatementTmplArg::Literal(v) => Some(v.clone()),
                StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
                _ => None,
            })
            .collect();
        if self.edb.custom_any_match(cpr, &filters) {
            min_body_cost = min_body_cost.min(1);
        }
        let remaining_native = goals
            .iter()
            .skip(current_idx + 1)
            .filter(|t| matches!(t.pred, Predicate::Native(_)))
            .count();
        let remaining_custom = goals
            .iter()
            .skip(current_idx + 1)
            .filter(|t| matches!(t.pred, Predicate::Custom(_)))
            .count();
        let extra_lb_ops = store.accumulated_lb_ops + min_body_cost;
        exceeds_best_pods_bound(
            &self.config,
            self.best_ops_so_far,
            self.best_inputs_so_far,
            realized_ops,
            realized_inputs,
            remaining_native,
            remaining_custom,
            extra_lb_ops,
        )
    }
}

#[inline]
fn record_head_step(
    store: &mut crate::types::ConstraintStore,
    head: pod2::middleware::Statement,
    tag: crate::types::OpTag,
) {
    store.premises.push((head, tag.clone()));
    store.operation_count = store.operation_count.saturating_add(1);
    if let crate::types::OpTag::CopyStatement { source } = tag {
        store.input_pods.insert(source);
    }
}

#[inline]
fn log_if_slow(label: &str, start: std::time::Instant, threshold_ms: u128) {
    let dt = start.elapsed();
    if dt.as_millis() > threshold_ms {
        trace!(ms = dt.as_millis(), label);
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn exceeds_best_pods_bound(
    cfg: &EngineConfig,
    best_ops: Option<usize>,
    best_inputs: Option<usize>,
    realized_ops: usize,
    realized_inputs: usize,
    remaining_native: usize,
    remaining_custom: usize,
    extra_lb_ops: usize,
) -> bool {
    let lb_ops = realized_ops + extra_lb_ops + remaining_native + remaining_custom;
    let ops_per_pod = cfg.ops_per_pod.max(1);
    let inputs_per_pod = cfg.inputs_per_pod.max(1);
    let pods_lb_ops = lb_ops.div_ceil(ops_per_pod);
    let pods_lb_inputs = realized_inputs.div_ceil(inputs_per_pod);
    let pods_lb = std::cmp::max(pods_lb_ops, pods_lb_inputs);
    match (best_ops, best_inputs) {
        (Some(bo), Some(bi)) => {
            let best_pods = std::cmp::max(bo.div_ceil(ops_per_pod), bi.div_ceil(inputs_per_pod));
            pods_lb > best_pods
        }
        _ => false,
    }
}

#[inline]
fn select_answers_for_waiter(
    table: &Table,
    waiter: &Waiter,
    cap: u32,
    delivered_this_epoch: u32,
) -> (Vec<(Vec<RawOrdValue>, crate::types::OpTag)>, u32, bool) {
    let mut budget_left = cap.saturating_sub(delivered_this_epoch);
    let mut to_deliver: Vec<(Vec<RawOrdValue>, crate::types::OpTag)> = Vec::new();
    if budget_left == 0 {
        return (to_deliver, 0, cap != u32::MAX);
    }
    for (tuple, tags) in table.answers.iter() {
        if budget_left == 0 {
            break;
        }
        if waiter.matches(tuple) {
            for tag in tags.iter() {
                if budget_left == 0 {
                    break;
                }
                to_deliver.push((tuple.clone(), tag.clone()));
                budget_left -= 1;
            }
        }
    }
    let inc = to_deliver.len() as u32;
    let exceeded = budget_left == 0 && cap != u32::MAX;
    (to_deliver, inc, exceeded)
}

#[inline]
fn select_waiters_for_answer(
    table: &Table,
    key_vec: &[RawOrdValue],
    cap: u32,
    delivered_this_epoch: u32,
) -> (Vec<Waiter>, u32, bool) {
    let mut budget_left = cap.saturating_sub(delivered_this_epoch);
    let mut to_deliver: Vec<Waiter> = Vec::new();
    if budget_left == 0 {
        return (to_deliver, 0, cap != u32::MAX);
    }
    for w in table.waiters.iter().cloned() {
        if budget_left == 0 {
            break;
        }
        if w.matches(key_vec) {
            to_deliver.push(w);
            budget_left -= 1;
        }
    }
    let inc = to_deliver.len() as u32;
    let exceeded = budget_left == 0 && cap != u32::MAX;
    (to_deliver, inc, exceeded)
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
    pub early_exit_on_first_answer: bool,
    pub branch_and_bound_on_ops: bool,
    // POD packing limits
    pub ops_per_pod: usize,
    pub inputs_per_pod: usize,
    pub wall_clock_timeout: Option<Duration>,
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
    pub fn early_exit_on_first_answer(mut self, enabled: bool) -> Self {
        self.cfg.early_exit_on_first_answer = enabled;
        self
    }
    pub fn branch_and_bound_on_ops(mut self, enabled: bool) -> Self {
        self.cfg.branch_and_bound_on_ops = enabled;
        self
    }
    pub fn ops_per_pod(mut self, ops: usize) -> Self {
        self.cfg.ops_per_pod = ops;
        self
    }
    pub fn inputs_per_pod(mut self, inputs: usize) -> Self {
        self.cfg.inputs_per_pod = inputs;
        self
    }
    pub fn from_params(mut self, params: &pod2::middleware::Params) -> Self {
        self.cfg.ops_per_pod = params.max_statements;
        self.cfg.inputs_per_pod = params.max_input_pods;
        self
    }
    pub fn wall_clock_timeout(mut self, timeout: Duration) -> Self {
        self.cfg.wall_clock_timeout = Some(timeout);
        self
    }
    pub fn wall_clock_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.cfg.wall_clock_timeout = Some(Duration::from_millis(timeout_ms));
        self
    }
    /// Apply recommended, bounded defaults and wire limits from Params.
    /// These are conservative, non-tight caps to prevent runaway work in no-solution cases.
    pub fn recommended(mut self, params: &pod2::middleware::Params) -> Self {
        // Wire POD packing limits
        self = self.from_params(params);
        // Bounded execution defaults
        self.cfg.iteration_cap = Some(100_000);
        self.cfg.per_table_fanout_cap = Some(1024);
        self.cfg.per_table_epoch_frames = Some(1_000);
        self.cfg.per_frame_step_cap = Some(1_000);
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
                // Custom statements cannot have anchored keys as arguments
                StatementTmplArg::AnchoredKey(_, _) | StatementTmplArg::None => {
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

    fn same_signature(&self, other: &Waiter) -> bool {
        self.pred == other.pred
            && self.goal_idx == other.goal_idx
            && self.bind_targets == other.bind_targets
            && self.literal_filters == other.literal_filters
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
        record_head_step(&mut cont_store, head_stmt, head_tag);

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

#[derive(Clone, PartialEq, Eq)]
pub struct CallPattern {
    pub(crate) pred: pod2::middleware::CustomPredicateRef,
    // For each head position, Some(literal) or None (variable/AK)
    pub(crate) literals: Vec<Option<RawOrdValue>>,
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
    // Deterministic map: head tuple -> one or more proof tags for the same logical answer
    answers: std::collections::BTreeMap<Vec<RawOrdValue>, Vec<crate::types::OpTag>>,
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

#[cfg(test)]
mod tests {
    use pod2::{
        lang::parse,
        middleware::{containers::Dictionary, Key, Params, Statement, Value},
    };
    use tracing_subscriber::{fmt, EnvFilter};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
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
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        // Registry with Equal and Lt handlers
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);
        crate::handlers::lteq::register_lteq_handlers(&mut reg);

        // Build goals via parser: Equal(R["k"], 1) and Lt(R["x"], 10)
        let processed = parse(
            r#"REQUEST(
                Equal(R["k"], 1)
                Lt(R["x"], 10)
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
        engine.run().expect("run ok");

        assert!(!engine.answers.is_empty());
        // At least one answer should bind wildcard 0 to the correct root
        let any_matches = engine.answers.iter().any(|store| {
            store
                .bindings
                .get(&0)
                .map(|v| v.raw() == Value::from(root).raw())
                .unwrap_or(false)
        });
        assert!(any_matches, "no answer bound R to the expected root");

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
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let processed = parse(
            r#"REQUEST(
                Equal(R["k"], 1)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        // Set a very small iteration cap to force early abort
        engine.config.iteration_cap = Some(0);
        engine.run().expect_err("iteration cap to be hit");
        assert!(engine.iteration_cap_hit, "expected iteration cap to be hit");
        // May or may not have answers depending on timing; just assert no panic and flag set
    }

    #[test]
    fn engine_fair_delivery_interleaves_with_independent_goal() {
        // Many roots for k:1 to create a large table of answers, and a separate small goal Equal(S["x"],3).
        let params = Params::default();
        let mut builder = ImmutableEdbBuilder::new();
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
            builder = builder.add_full_dict(d);
        }
        // Add independent root S with x:3
        let d_s = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(3))].into(),
        )
        .unwrap();
        let root_s = d_s.commitment();
        let edb = builder.add_full_dict(d_s).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // Custom predicate enumerates all roots with k:1 via entries
        let program = r#"
            make_r(R) = AND(
                Equal(R["k"], 1)
            )

            REQUEST(
                make_r(R)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        // Also enqueue an independent goal Equal(S["x"], 3)
        let processed2 = parse(
            r#"REQUEST(
                Equal(S["x"], 3)
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

        engine.run().expect("run ok");

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
            "independent Equal(S[\"x\"],3) should complete under fanout caps"
        );
    }

    #[test]
    fn scheduler_policy_depth_first_vs_breadth_first() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
        // Build two trivial frames with prepopulated bindings and no goals; check answer order
        let edb = ImmutableEdbBuilder::new().build();
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
        eng_dfs.run().expect("run ok");
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
        eng_bfs.run().expect("run ok");
        assert_eq!(eng_bfs.answers.len(), 2);
        assert_eq!(eng_bfs.answers[0].bindings.get(&0), Some(&Value::from(1)));
        assert_eq!(eng_bfs.answers[1].bindings.get(&0), Some(&Value::from(2)));
    }

    #[test]
    fn determinism_golden_many_choices() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
        // Build 5 roots each with k:1; query Equal(R["k"], 1). Ordering should be stable across runs.
        let params = Params::default();
        let mut builder = ImmutableEdbBuilder::new();
        let mut roots = Vec::new();
        for _i in 0..5 {
            let key = Key::from("k");
            let dict = Dictionary::new(
                params.max_depth_mt_containers,
                [(key.clone(), Value::from(1))].into(),
            )
            .unwrap();
            let r = dict.commitment();
            builder = builder.add_full_dict(dict);
            roots.push(r);
        }
        let edb = builder.build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let processed = parse(
            r#"REQUEST(
                Equal(R["k"], 1)
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
        engine1.run().expect("run ok");
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
        engine2.run().expect("run ok");
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
        // Parent has Lt(A, 20); subcall binds A via Equal from entries
        let params = Params::default();
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
        let edb = ImmutableEdbBuilder::new()
            .add_full_dict(d_ok)
            .add_full_dict(d_bad)
            .build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);
        register_lteq_handlers(&mut reg);

        // Define helper AND that ties A to R["x"], then call it under top-level Lt(A,20)
        // and Equal(R["x"], 15) to ground the subcall.
        let input = r#"
            helper(A, R) = AND(
                Equal(R["x"], A)
            )

            REQUEST(
                Lt(A, 20)
                Equal(R["x"], 15)
                helper(A, R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

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
        let d = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(10))].into(),
        )
        .unwrap();
        let edb = ImmutableEdbBuilder::new().add_full_dict(d).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);

        // The Lt(Z, 5) constraint mentions Z which is not in helper's head → must not be propagated
        let input = r#"
            helper(A, R) = AND(
                Equal(R["x"], A)
            )

            REQUEST(
                Lt(Z, 5)
                helper(A, R)
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
        // The first goal should be the body Equal, not the unrelated Lt(Z,5)
        let Frame { goals, .. } = frame;
        // The propagated list should not include Lt(Z,5) since Z is not in helper head
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
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);

        // Lt first (suspends), Equal second (binds root)
        let processed = parse(
            r#"REQUEST(
                Lt(R["x"], 10)
                Equal(R["k"], 1)
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
        engine.run().expect("run ok");

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
        assert!(any_matches, "no answer bound R to expected root");

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
        // Single goal: Lt(R["x"], 10) with no other goal to bind R → should park the frame
        let edb = ImmutableEdbBuilder::new().build();
        let mut reg = OpRegistry::default();
        register_lt_handlers(&mut reg);
        let processed = parse(
            r#"REQUEST(
                Lt(R["x"], 10)
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
        engine.run().expect_err("should not produce an answer");

        assert!(engine.answers.is_empty(), "should not produce an answer");
        assert_eq!(
            engine.sched.parked.len(),
            1,
            "frame should be parked waiting on R"
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
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // Single goal Equal(R["k"], 1) should bind R to root. Two internal choices exist;
        // engine must dedup and prefer the GeneratedContains-based proof.
        let processed = parse(
            r#"REQUEST(
                Equal(R["k"], 1)
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
        engine.run().expect("run ok");

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
        let builder = ImmutableEdbBuilder::new();
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
        let edb = builder.add_full_dict(dict_r).add_full_dict(dict_c).build();

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
                Lt(A, 50)
                Equal(R["some_key"], A)
                Equal(C["other_key"], A)
                SumOf(R["some_key"], 19, 1)
            )

            REQUEST(
                my_pred(A, R, C)
            )
        "#;
        let processed2 = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        // Load and enqueue via helper
        engine.load_processed(&processed2);
        let cpr = CustomPredicateRef::new(processed2.custom_batch.clone(), 0);
        engine.run().expect("run ok");

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
        let mut builder = ImmutableEdbBuilder::new();
        let d1 = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("a"), Value::from(1))].into(),
        )
        .unwrap();
        let r1 = d1.commitment();
        builder = builder.add_full_dict(d1);
        let d2 = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("a"), Value::from(2))].into(),
        )
        .unwrap();
        let r2 = d2.commitment();
        let edb = builder.add_full_dict(d2).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // Define disjunctive predicate and request
        let input = r#"
            my_pred(R) = OR(
                Equal(R["a"], 1)
                Equal(R["a"], 2)
            )

            REQUEST(
                my_pred(R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        let cpr = CustomPredicateRef::new(processed.custom_batch.clone(), 0);
        engine.run().expect("run ok");

        // Expect two answers binding R to r1 and r2
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
        let _ = env_logger::builder().is_test(true).try_init();
        // Root has x:7
        let d = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("x"), Value::from(7))].into(),
        )
        .unwrap();
        let r = d.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(d).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        // helper(R) = AND(Equal(R["x"], 7))
        // my_pred(R) = OR(helper(R), Equal(R["x"], 8))
        let input = r#"
            helper(R) = AND(
                Equal(R["x"], 7)
            )

            my_pred(R) = OR(
                helper(R)
                Equal(R["x"], 8)
            )

            REQUEST(
                my_pred(R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

        assert!(!engine.answers.is_empty());
        let ans = &engine.answers[0];
        assert_eq!(
            ans.bindings.get(&0).map(|v| v.raw()),
            Some(Value::from(r).raw())
        );
    }

    #[test]
    fn engine_with_immutable_edb_equal_from_entries() {
        // Build an immutable EDB with a full dictionary containing k:1 and prove Equal(R["k"], 1)
        use crate::edb::ImmutableEdbBuilder;

        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("k"), Value::from(1))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let processed = parse(
            r#"REQUEST(
                Equal(R["k"], 1)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

        assert!(!engine.answers.is_empty());
        let st = &engine.answers[0];
        assert_eq!(
            st.bindings.get(&0).map(|v| v.raw()),
            Some(Value::from(root).raw())
        );
    }

    #[test]
    fn engine_with_immutable_edb_equal_from_signed_dict() {
        // Build an immutable EDB with a signed dictionary containing k:1 and prove Equal(R["k"], 1)
        use crate::edb::ImmutableEdbBuilder;

        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("k"), Value::from(1))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let processed = parse(
            r#"REQUEST(
                Equal(R["k"], 1)
            )"#,
            &Params::default(),
            &[],
        )
        .expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

        assert!(!engine.answers.is_empty());
        let st = &engine.answers[0];
        assert_eq!(
            st.bindings.get(&0).map(|v| v.raw()),
            Some(Value::from(root).raw())
        );
    }

    #[test]
    fn engine_custom_edb_copy_only_streams() {
        use pod2::middleware::{CustomPredicateRef, Value as V};

        // Define a predicate that we cannot deduce for A=10 via its rule, but exists in EDB as a custom row.
        let program = r#"
            my_pred(A) = AND(
                Equal(A, 9999) // prevents rule-based deduction for A=10
            )

            REQUEST(
                my_pred(10)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let cpr = CustomPredicateRef::new(processed.custom_batch.clone(), 0);

        // EDB with a custom head my_pred(10) copied from some PodRef
        let fake_src = crate::types::PodRef(pod2::middleware::Hash::from(V::from(42).raw()));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::Custom(cpr.clone(), vec![V::from(10)]), fake_src)
            .build();

        // No handlers needed for the failing Equal(A,9999) since it won't match
        let reg = OpRegistry::default();

        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

        assert!(
            !engine.answers.is_empty(),
            "expected at least one answer via EDB Copy"
        );

        // Verify that we got a Custom head proved via CopyStatement
        let mut saw_copy = false;
        for st in engine.answers.iter() {
            for (stmt, tag) in st.premises.iter() {
                if let pod2::middleware::Statement::Custom(pred, vals) = stmt {
                    if *pred == cpr
                        && vals.len() == 1
                        && vals[0] == V::from(10)
                        && matches!(tag, crate::types::OpTag::CopyStatement { .. })
                    {
                        saw_copy = true;
                    }
                }
            }
        }
        assert!(saw_copy, "expected CopyStatement proof for my_pred(10)");
    }

    #[test]
    fn engine_custom_edb_and_rule_both_stream() {
        use pod2::middleware::{CustomPredicateRef, Value as V};

        // Predicate can be deduced (A bound by SumOf), and also exists in the EDB.
        let program = r#"
            my_pred(A) = AND(
                SumOf(A, 7, 3)
            )

            REQUEST(
                my_pred(10)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let cpr = CustomPredicateRef::new(processed.custom_batch.clone(), 0);

        // EDB custom row for my_pred(10)
        let fake_src = crate::types::PodRef(pod2::middleware::Hash::from(V::from(77).raw()));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::Custom(cpr.clone(), vec![V::from(10)]), fake_src)
            .build();

        // Handlers to allow rule-based deduction via SumOf
        let mut reg = OpRegistry::default();
        register_sumof_handlers(&mut reg);

        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

        assert!(!engine.answers.is_empty(), "expected at least one answer");

        // Expect at least one CopyStatement proof and at least one CustomDeduction proof for my_pred(10)
        let mut saw_copy = false;
        let mut saw_custom = false;
        for st in engine.answers.iter() {
            for (stmt, tag) in st.premises.iter() {
                if let pod2::middleware::Statement::Custom(pred, vals) = stmt {
                    if *pred == cpr && vals.len() == 1 && vals[0] == V::from(10) {
                        match tag {
                            crate::types::OpTag::CopyStatement { .. } => saw_copy = true,
                            crate::types::OpTag::CustomDeduction { .. } => saw_custom = true,
                            _ => {}
                        }
                    }
                }
            }
        }
        assert!(saw_copy, "expected a CopyStatement proof for my_pred(10)");
        assert!(
            saw_custom,
            "expected a CustomDeduction proof for my_pred(10) from the rule"
        );
    }

    #[test]
    fn engine_custom_or_rejects_self_recursion() {
        // Bad(R) = OR(Bad(R), Equal(R["y"], 1)) should reject the recursive branch and still solve via Equal
        let params = Params::default();
        let d = Dictionary::new(
            params.max_depth_mt_containers,
            [(Key::from("y"), Value::from(1))].into(),
        )
        .unwrap();
        let r = d.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(d).build();

        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);

        let input = r#"
            Bad(R) = OR(
                Bad(R)
                Equal(R["y"], 1)
            )

            REQUEST(
                Bad(R)
            )
        "#;
        let processed = parse(input, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

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
        let edb = ImmutableEdbBuilder::new().build();
        let reg = OpRegistry::default();

        // Define a self-recursive AND predicate and call it.
        let program = r#"
            bad(A) = AND(
                bad(A)
            )

            REQUEST(
                bad(1)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::new(&reg, &edb);
        // Register rules (self-recursive AND is rejected → no rules for 'bad') and enqueue request
        engine.load_processed(&processed);
        engine.run().expect_err("should not produce an answer");

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
        let edb = ImmutableEdbBuilder::new().build();
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_lt_handlers(&mut reg);
        register_sumof_handlers(&mut reg);

        let program = r#"
            dec(A, B) = AND(
                SumOf(A, B, 1)
            )

            step(N, private: M) = AND(
                Lt(0, N)
                dec(N, M)
                nat_down(M)
            )

            nat_down(N) = OR(
                Equal(N, 0)
                step(N)
            )

            REQUEST(
                nat_down(3)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let config = EngineConfigBuilder::new()
            .recommended(&Params::default())
            .build();
        let mut engine = Engine::with_config(&reg, &edb, config);
        engine.load_processed(&processed);
        engine.run().expect("run ok");

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
        let edb = ImmutableEdbBuilder::new().build();
        let mut reg = OpRegistry::default();
        register_equal_handlers(&mut reg);
        register_sumof_handlers(&mut reg);
        register_lt_handlers(&mut reg);

        let program = r#"
            dec(A, B) = AND(
                SumOf(A, B, 1)
            )

            even_step(N, private: M) = AND(
                Lt(0, N)
                dec(N, M)
                odd(M)
            )

            even(N) = OR(
                Equal(N, 0)
                even_step(N)
            )

            odd(N, private: M) = AND(
                Lt(0, N)
                dec(N, M)
                even(M)
            )

            REQUEST(
                even(4)
            )
        "#;
        let processed = parse(program, &Params::default(), &[]).expect("parse ok");
        let mut engine = Engine::with_config(
            &reg,
            &edb,
            EngineConfigBuilder::new()
                .early_exit_on_first_answer(true)
                .build(),
        );
        engine.load_processed(&processed);
        engine.run().expect("run ok");

        assert!(
            !engine.answers.is_empty(),
            "expected at least one answer proving even(4)"
        );
    }
}
