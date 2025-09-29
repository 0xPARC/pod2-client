use std::{collections::HashMap, sync::Arc};

use pod2::middleware::{CustomPredicate, CustomPredicateBatch, CustomPredicateRef, Params};

use crate::{builder::Obj, guards::Ctx, lowering::build_predicate_in_batch};

pub struct BatchBuilder<'a> {
    params: &'a Params,
    items: Vec<(String, Box<dyn Fn(&mut Ctx) -> Obj + 'static>)>,
}

impl<'a> BatchBuilder<'a> {
    pub fn new(params: &'a Params) -> Self {
        Self {
            params,
            items: Vec::new(),
        }
    }

    pub fn add<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&mut Ctx) -> Obj + 'static,
    {
        self.items.push((name.into(), Box::new(f)));
    }

    /// Build a `CustomPredicateBatch` from the added reducers and return a mapping
    /// from reducer name to `CustomPredicateRef` for external references.
    pub fn build(
        self,
        batch_name: impl Into<String>,
    ) -> (
        Arc<CustomPredicateBatch>,
        HashMap<String, CustomPredicateRef>,
    ) {
        let name_to_index: HashMap<String, usize> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, (n, _))| (n.clone(), i))
            .collect();

        let mut preds: Vec<CustomPredicate> = Vec::with_capacity(self.items.len());
        for (name, f) in self.items.into_iter() {
            let pred = build_predicate_in_batch(self.params, &name, |ctx| (f)(ctx), &name_to_index);
            preds.push(pred);
        }

        let batch = CustomPredicateBatch::new(self.params, batch_name.into(), preds);

        let mut refs: HashMap<String, CustomPredicateRef> = HashMap::new();
        for (name, idx) in name_to_index.into_iter() {
            refs.insert(name, CustomPredicateRef::new(batch.clone(), idx));
        }
        (batch, refs)
    }
}
