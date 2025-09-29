#[macro_export]
macro_rules! reducer {
    ($name:ident, $tag:expr, |$ctx:ident| $body:block) => {
        pub mod $name {
            pub const TAG: &str = $tag;
            pub fn predicate(
                params: &pod2::middleware::Params,
            ) -> pod2::middleware::CustomPredicate {
                $crate::build_predicate(params, $tag, |$ctx: &mut $crate::Ctx| $body).predicate
            }
            pub fn exec(
                prev: &pod2::middleware::containers::Dictionary,
                action: &pod2::middleware::containers::Dictionary,
            ) -> $crate::Result<pod2::middleware::containers::Dictionary> {
                let mut __ctx = $crate::Ctx::new();
                let __expr = (|$ctx: &mut $crate::Ctx| -> $crate::Obj { $body })(&mut __ctx);
                $crate::exec_reducer(prev, action, __expr.expr(), __ctx.guards())
            }
            pub fn plan(
                params: &pod2::middleware::Params,
                prev: &pod2::middleware::containers::Dictionary,
                action: &pod2::middleware::containers::Dictionary,
                next: &pod2::middleware::containers::Dictionary,
            ) -> $crate::Result<Vec<pod2::frontend::Operation>> {
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
            use pod2::middleware::{containers::Dictionary, Params, TypedValue, Key};

            pub const TAGS: &[&str] = &[ $( $tag ),+ ];

            fn action_tag(action: &Dictionary) -> $crate::Result<String> {
                let v = action.get(&Key::from("action_type"))
                    .map_err($crate::Error::from)?;
                match v.typed() {
                    TypedValue::String(s) => Ok(s.clone()),
                    other => Err($crate::Error::Type(format!("action_type must be string, got {other}"))),
                }
            }

            pub fn exec(
                prev: &Dictionary,
                action: &Dictionary,
            ) -> $crate::Result<Dictionary> {
                let tag = action_tag(action)?;
                match tag.as_str() {
                    $( $tag => super::$mod::exec(prev, action), )+
                    _ => Err($crate::Error::Msg(format!("Unknown action_type tag: {}", tag)))
                }
            }

            pub fn plan(
                params: &Params,
                prev: &Dictionary,
                action: &Dictionary,
                next: &Dictionary,
            ) -> $crate::Result<Vec<pod2::frontend::Operation>> {
                let tag = action_tag(action)?;
                match tag.as_str() {
                    $( $tag => super::$mod::plan(params, prev, action, next), )+
                    _ => Err($crate::Error::Msg(format!("Unknown action_type tag: {}", tag)))
                }
            }

        }
    };
}

/// Convenience macro to build an Action dictionary with an `action_type` and extra fields.
/// Variants:
/// - action!(tag, { k1 => v1, k2 => v2, ... }) uses default Params
/// - action!(params, tag, { ... }) uses provided Params (recommended in examples)
#[macro_export]
macro_rules! action {
    ($tag:expr, { $( $k:expr => $v:expr ),* $(,)? }) => {{
        pod2::dict!(pod2::middleware::Params::default().max_depth_mt_containers, {
            "action_type" => $tag,
            $( $k => $v ),*
        }).expect("failed to build action dictionary")
    }};
    ($params:expr, $tag:expr, { $( $k:expr => $v:expr ),* $(,)? }) => {{
        pod2::dict!($params.max_depth_mt_containers, {
            "action_type" => $tag,
            $( $k => $v ),*
        }).expect("failed to build action dictionary")
    }};
}
