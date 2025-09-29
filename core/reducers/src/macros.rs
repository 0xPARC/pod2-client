#[macro_export]
macro_rules! reducer {
    ($name:ident, $tag:expr, |$ctx:ident| $body:block) => {
        pub mod $name {
            pub fn predicate(
                params: &pod2::middleware::Params,
            ) -> pod2::middleware::CustomPredicate {
                $crate::build_predicate(params, $tag, |$ctx: &mut $crate::Ctx| $body).predicate
            }
            pub fn exec(
                prev: &pod2::middleware::containers::Dictionary,
                action: &pod2::middleware::containers::Dictionary,
            ) -> $crate::error::Result<pod2::middleware::containers::Dictionary> {
                let mut __ctx = $crate::Ctx::new();
                let __expr = (|$ctx: &mut $crate::Ctx| -> $crate::Obj { $body })(&mut __ctx);
                $crate::exec_reducer(prev, action, __expr.expr(), &__ctx.guards)
            }
            pub fn plan(
                params: &pod2::middleware::Params,
                prev: &pod2::middleware::containers::Dictionary,
                action: &pod2::middleware::containers::Dictionary,
                next: &pod2::middleware::containers::Dictionary,
            ) -> $crate::error::Result<Vec<pod2::frontend::Operation>> {
                let pred =
                    $crate::build_predicate(params, $tag, |$ctx: &mut $crate::Ctx| $body).predicate;
                let mut bindings = std::collections::HashMap::new();
                bindings.insert(
                    "Prev".to_string(),
                    pod2::middleware::Value::from(prev.clone()),
                );
                bindings.insert(
                    "Action".to_string(),
                    pod2::middleware::Value::from(action.clone()),
                );
                bindings.insert(
                    "Next".to_string(),
                    pod2::middleware::Value::from(next.clone()),
                );
                $crate::plan_operations(&pred, params, &bindings)
            }
        }
    };
}

#[macro_export]
macro_rules! reducer_router {
    ($name:ident, [ $( $tag:expr => $mod:ident ),+ $(,)? ]) => {
        pub mod $name {
            use std::collections::HashMap;
            use std::sync::Arc;
            use pod2::middleware::{containers::Dictionary, Params, CustomPredicateBatch, CustomPredicateRef, TypedValue, Key};

            pub const TAGS: &[&str] = &[ $( $tag ),+ ];

            fn action_tag(action: &Dictionary) -> $crate::error::Result<String> {
                let v = action.get(&Key::from("action_type"))
                    .map_err($crate::error::Error::from)?;
                match v.typed() {
                    TypedValue::String(s) => Ok(s.clone()),
                    other => Err($crate::error::Error::Type(format!("action_type must be string, got {other}"))),
                }
            }

            pub fn exec(
                prev: &Dictionary,
                action: &Dictionary,
            ) -> $crate::error::Result<Dictionary> {
                let tag = action_tag(action)?;
                match tag.as_str() {
                    $( $tag => super::$mod::exec(prev, action), )+
                    _ => Err($crate::error::Error::Msg(format!("Unknown action_type tag: {}", tag)))
                }
            }

            pub fn plan(
                params: &Params,
                prev: &Dictionary,
                action: &Dictionary,
                next: &Dictionary,
            ) -> $crate::error::Result<Vec<pod2::frontend::Operation>> {
                let tag = action_tag(action)?;
                match tag.as_str() {
                    $( $tag => super::$mod::plan(params, prev, action, next), )+
                    _ => Err($crate::error::Error::Msg(format!("Unknown action_type tag: {}", tag)))
                }
            }

            /// Build a batch that includes all branch predicates and return refs keyed by tag
            pub fn batch(params: &Params) -> (Arc<CustomPredicateBatch>, HashMap<String, CustomPredicateRef>) {
                let preds: Vec<pod2::middleware::CustomPredicate> = vec![ $( super::$mod::predicate(params) ),+ ];
                let batch = CustomPredicateBatch::new(params, stringify!($name).to_string(), preds);
                let mut refs: HashMap<String, CustomPredicateRef> = HashMap::new();
                let mut i = 0usize;
                $( { refs.insert($tag.to_string(), CustomPredicateRef::new(batch.clone(), i)); i += 1; } )+;
                (batch, refs)
            }
        }
    };
}
