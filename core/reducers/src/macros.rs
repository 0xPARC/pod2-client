#[macro_export]
macro_rules! reducer {
    ($name:ident, $tag:expr, |$ctx:ident| $body:block) => {
        #[allow(dead_code, unused_variables, unused_imports, unused_assignments)]
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
            pub fn exec_with_aux(
                prev: &pod2::middleware::containers::Dictionary,
                action: &pod2::middleware::containers::Dictionary,
                aux: &$crate::ExecAux,
            ) -> $crate::Result<pod2::middleware::containers::Dictionary> {
                let mut __ctx = $crate::Ctx::new();
                let __expr = (|$ctx: &mut $crate::Ctx| -> $crate::Obj { $body })(&mut __ctx);
                $crate::exec_reducer_with_aux(prev, action, __expr.expr(), __ctx.guards(), aux)
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
            pub fn plan_signed(
                params: &pod2::middleware::Params,
                prev: &pod2::middleware::containers::Dictionary,
                signed: &pod2::frontend::SignedDict,
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
                    pod2::middleware::Value::from(signed.dict.clone()),
                );
                bindings.insert(
                    "Next".to_string(),
                    pod2::middleware::Value::from(next.clone()),
                );
                let ops = $crate::plan_operations(&pred, params, &bindings)?;
                $crate::attach_signature_to_ops(ops, signed)
            }
        }
    };
}

#[macro_export]
macro_rules! reducer_router {
    ($name:ident, [ $( $tag:expr => $mod:ident ),+ $(,)? ]) => {
        #[allow(dead_code, unused_variables, unused_imports, unused_assignments)]
        pub mod $name {
            use pod2::middleware::{
                containers::Dictionary,
                Params, TypedValue, Key,
                Predicate, StatementTmpl, StatementTmplArg, CustomPredicate, CustomPredicateBatch, CustomPredicateRef, Wildcard,
                MainPodProver, VDSet,
            };
            use pod2::frontend::{SignedDict, MainPodBuilder, Operation as FOperation, MainPod};
            use pod2::middleware::Statement as MwStatement;

            pub const TAGS: &[&str] = &[ $( $tag ),+ ];

            fn action_tag(action: &Dictionary) -> $crate::Result<String> {
                let v = action.get(&Key::from("action_type"))
                    .map_err($crate::Error::from)?;
                match v.typed() {
                    TypedValue::String(s) => Ok(s.clone()),
                    other => Err($crate::Error::Type(format!("action_type must be string, got {other}"))),
                }
            }

            /// Build a top-level CustomPredicate that is an OR over all branch reducers.
            /// The predicate takes public args (Prev, Next) and private arg (Action). It branches to
            /// the corresponding reducer predicate based on `action_type` at verification time.
            pub fn predicate(params: &Params) -> CustomPredicate {
                // Build the batch with all branch predicates so we can reference them
                let preds = vec![ $( super::$mod::predicate(params) ),+ ];
                let batch = CustomPredicateBatch::new(params, stringify!($name).to_string(), preds);

                // Wildcards order so Action is private: [Prev, Next, Action]; args_len = 2
                let prev = Wildcard::new("Prev".to_string(), 0);
                let next = Wildcard::new("Next".to_string(), 1);
                let action = Wildcard::new("Action".to_string(), 2);

                // We need correct indices for each arm. Rebuild with explicit indices.
                // Since macros can't easily enumerate, build manually here:
                let mut stmts_indexed: Vec<StatementTmpl> = Vec::new();
                let mut idx = 0usize;
                $( {
                    let _ = $tag; // tie repetition to macro metavariables
                    let pred_ref = CustomPredicateRef::new(batch.clone(), idx);
                    let st = StatementTmpl {
                        pred: Predicate::Custom(pred_ref),
                        args: vec![
                            StatementTmplArg::Wildcard(prev.clone()),
                            StatementTmplArg::Wildcard(action.clone()),
                            StatementTmplArg::Wildcard(next.clone()),
                        ],
                    };
                    stmts_indexed.push(st);
                    idx += 1;
                } )+

                CustomPredicate::or(
                    params,
                    format!("{}", stringify!($name)),
                    stmts_indexed,
                    2,
                    vec!["Prev".into(), "Next".into(), "Action".into()]
                ).expect("valid router OR predicate")
            }

            /// Build a batch containing all branch predicates plus the router OR predicate
            /// (as the last entry). Useful to obtain `CustomPredicateRef`s for both.
            pub fn predicate_batch(params: &Params) -> std::sync::Arc<CustomPredicateBatch> {
                // Branch predicates first
                let mut preds = vec![ $( super::$mod::predicate(params) ),+ ];
                // Build OR using BatchSelf indices over the first N entries
                let prev = Wildcard::new("Prev".to_string(), 0);
                let next = Wildcard::new("Next".to_string(), 1);
                let action = Wildcard::new("Action".to_string(), 2);
                let mut idx = 0usize;
                let mut stmts: Vec<StatementTmpl> = Vec::new();
                $( {
                    let _ = $tag; // tie repetition
                    let st = StatementTmpl {
                        pred: Predicate::BatchSelf(idx),
                        args: vec![
                            StatementTmplArg::Wildcard(prev.clone()),
                            StatementTmplArg::Wildcard(action.clone()),
                            StatementTmplArg::Wildcard(next.clone()),
                        ],
                    };
                    stmts.push(st);
                    idx += 1;
                } )+
                let or_pred = CustomPredicate::or(
                    params,
                    format!("{}", stringify!($name)),
                    stmts,
                    2,
                    vec!["Prev".into(), "Next".into(), "Action".into()]
                ).expect("valid router OR predicate");
                preds.push(or_pred);
                CustomPredicateBatch::new(params, stringify!($name).to_string(), preds)
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
            pub fn exec_signed(
                prev: &Dictionary,
                signed: &SignedDict,
            ) -> $crate::Result<Dictionary> {
                let aux = $crate::ExecAux { signed_action: Some(signed.clone()) };
                let tag = action_tag(&signed.dict)?;
                match tag.as_str() {
                    $( $tag => super::$mod::exec_with_aux(prev, &signed.dict, &aux), )+
                    _ => Err($crate::Error::Msg(format!("Unknown action_type tag: {}", tag)))
                }
            }
            pub fn exec_with_aux(
                prev: &Dictionary,
                action: &Dictionary,
                aux: &$crate::ExecAux,
            ) -> $crate::Result<Dictionary> {
                let tag = action_tag(action)?;
                match tag.as_str() {
                    $( $tag => super::$mod::exec_with_aux(prev, action, aux), )+
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
            pub fn plan_signed(
                params: &Params,
                prev: &Dictionary,
                signed: &SignedDict,
                next: &Dictionary,
            ) -> $crate::Result<Vec<pod2::frontend::Operation>> {
                let tag = action_tag(&signed.dict)?;
                match tag.as_str() {
                    $( $tag => super::$mod::plan_signed(params, prev, signed, next), )+
                    _ => Err($crate::Error::Msg(format!("Unknown action_type tag: {}", tag)))
                }
            }

            /// Build a POD proving the selected branch under the router OR predicate, with only the
            /// router-level custom predicate public. All native statements are private.
            pub fn build_pod_signed(
                params: &Params,
                vd_set: &VDSet,
                prev: &Dictionary,
                signed: &SignedDict,
                next: &Dictionary,
                prover: &dyn MainPodProver,
            ) -> $crate::Result<MainPod> {
                let tag = action_tag(&signed.dict)?;
                let mut builder = MainPodBuilder::new(params, vd_set);

                // Plan ops and collect private statements for the matching branch
                // Also determine the branch index in the batch ordering
                let mut branch_idx_opt: Option<usize> = None;
                let mut idx = 0usize;
                let mut branch_statements: Vec<MwStatement> = Vec::new();
                $( {
                    let _ = $tag; // tie repetition
                    if branch_idx_opt.is_none() && tag.as_str() == $tag {
                        branch_idx_opt = Some(idx);
                        let ops = super::$mod::plan_signed(params, prev, signed, next)?;
                        for op in ops.into_iter() {
                            // Private native statements; collect for branch custom op
                            let st = builder.priv_op(op).map_err(|e| $crate::Error::Msg(e.to_string()))?;
                            branch_statements.push(st);
                        }
                    }
                    idx += 1;
                } )+
                let branch_idx = branch_idx_opt.ok_or_else(|| $crate::Error::Msg(format!(
                    "Unknown action_type tag: {}",
                    tag
                )))?;

                // Build predicate batch and refs
                let batch = predicate_batch(params);
                let cpr_branch = CustomPredicateRef::new(batch.clone(), branch_idx);
                let cpr_router = CustomPredicateRef::new(batch.clone(), TAGS.len());

                // Insert branch custom statement as private
                let st_branch = builder
                    .priv_op(FOperation::custom(cpr_branch, branch_statements))
                    .map_err(|e| $crate::Error::Msg(e.to_string()))?;

                // Build router OR op args: None for other branches, chosen branch gets st_branch
                let mut router_args: Vec<MwStatement> = Vec::with_capacity(TAGS.len());
                for i in 0..TAGS.len() {
                    if i == branch_idx { router_args.push(st_branch.clone()); } else { router_args.push(MwStatement::None); }
                }
                // Insert router-level custom statement as public
                let _st_router = builder
                    .pub_op(FOperation::custom(cpr_router, router_args))
                    .map_err(|e| $crate::Error::Msg(e.to_string()))?;

                builder.prove(prover).map_err(|e| $crate::Error::Msg(e.to_string()))
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

/// Build a SignedDict action with `action_type` and extra fields, signed by a signer.
/// Variants:
/// - signed_action!(params, signer, tag, { k1 => v1, ... })
#[macro_export]
macro_rules! signed_action {
    ($params:expr, $signer:expr, $tag:expr, { $( $k:expr => $v:expr ),* $(,)? }) => {{
        let mut __b = pod2::frontend::SignedDictBuilder::new(&$params);
        __b.insert("action_type", $tag);
        $( __b.insert($k, $v); )*
        // Note: this macro does NOT auto-insert a `public_key`/`user` field.
        // Include a signer identifier (e.g., `user`) yourself in the map
        // and reference it in reducers via `ctx.require_signed_by(&action, action.get("user"))`.
        __b.sign(&$signer).expect("failed to sign action dictionary")
    }};
}
