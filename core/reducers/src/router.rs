use std::collections::HashMap;

use pod2::{
    frontend::Operation,
    middleware::{containers::Dictionary, Key, Params, TypedValue},
};

use crate::error::{Error, Result};

type ExecFn = fn(&Dictionary, &Dictionary) -> Result<Dictionary>;
type PlanFn = fn(&Params, &Dictionary, &Dictionary, &Dictionary) -> Result<Vec<Operation>>;
#[derive(Clone, Copy)]
struct BranchFns {
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

    pub fn add(&mut self, tag: impl Into<String>, exec: ExecFn, plan: PlanFn) -> &mut Self {
        self.order.push((tag.into(), BranchFns { exec, plan }));
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

    // batch() removed in simplified API
}
