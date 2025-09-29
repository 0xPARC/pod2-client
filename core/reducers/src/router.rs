use std::{collections::HashMap, sync::Arc};

use pod2::{
    frontend::Operation,
    middleware::{
        containers::Dictionary, CustomPredicate, CustomPredicateBatch, CustomPredicateRef, Key,
        Params, TypedValue,
    },
};

use crate::error::{Error, Result};

type ExecFn = fn(&Dictionary, &Dictionary) -> Result<Dictionary>;
type PlanFn = fn(&Params, &Dictionary, &Dictionary, &Dictionary) -> Result<Vec<Operation>>;
type PredFn = fn(&Params) -> CustomPredicate;

#[derive(Clone, Copy)]
struct BranchFns {
    pred: PredFn,
    exec: ExecFn,
    plan: PlanFn,
}

pub struct RouterBuilder {
    order: Vec<(String, BranchFns)>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        Self { order: Vec::new() }
    }

    pub fn add(
        &mut self,
        tag: impl Into<String>,
        predicate: PredFn,
        exec: ExecFn,
        plan: PlanFn,
    ) -> &mut Self {
        self.order.push((
            tag.into(),
            BranchFns {
                pred: predicate,
                exec,
                plan,
            },
        ));
        self
    }

    pub fn build(self) -> Router {
        let mut index = HashMap::new();
        for (i, (tag, _)) in self.order.iter().enumerate() {
            index.insert(tag.clone(), i);
        }
        Router {
            branches: self.order,
            index,
        }
    }
}

pub struct Router {
    branches: Vec<(String, BranchFns)>,
    index: HashMap<String, usize>,
}

impl Router {
    fn action_tag(action: &Dictionary) -> Result<String> {
        let v = action.get(&Key::from("action_type")).map_err(Error::from)?;
        match v.typed() {
            TypedValue::String(s) => Ok(s.clone()),
            other => Err(Error::Type(format!(
                "action_type must be string, got {other}"
            ))),
        }
    }

    pub fn exec(&self, prev: &Dictionary, action: &Dictionary) -> Result<Dictionary> {
        let tag = Self::action_tag(action)?;
        let Some(&i) = self.index.get(&tag) else {
            return Err(Error::Msg(format!("Unknown action_type tag: {tag}")));
        };
        (self.branches[i].1.exec)(prev, action)
    }

    pub fn plan(
        &self,
        params: &Params,
        prev: &Dictionary,
        action: &Dictionary,
        next: &Dictionary,
    ) -> Result<Vec<Operation>> {
        let tag = Self::action_tag(action)?;
        let Some(&i) = self.index.get(&tag) else {
            return Err(Error::Msg(format!("Unknown action_type tag: {tag}")));
        };
        (self.branches[i].1.plan)(params, prev, action, next)
    }

    pub fn batch(
        &self,
        params: &Params,
    ) -> (
        Arc<CustomPredicateBatch>,
        HashMap<String, CustomPredicateRef>,
    ) {
        let mut preds: Vec<CustomPredicate> = Vec::with_capacity(self.branches.len());
        for (_, fns) in self.branches.iter() {
            preds.push((fns.pred)(params));
        }
        let batch = CustomPredicateBatch::new(params, "RouterBatch".to_string(), preds);
        let mut refs = HashMap::new();
        for (i, (tag, _)) in self.branches.iter().enumerate() {
            refs.insert(tag.clone(), CustomPredicateRef::new(batch.clone(), i));
        }
        (batch, refs)
    }
}
