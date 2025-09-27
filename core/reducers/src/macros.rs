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
