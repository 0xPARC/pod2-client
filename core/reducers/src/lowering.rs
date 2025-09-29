use std::collections::HashMap;

use pod2::middleware::{
    Key, NativePredicate, Params, Predicate, StatementTmpl, StatementTmplArg, Value, Wildcard,
};

use crate::{
    ast::{Expr, VarKind},
    builder::Obj,
    guards::{Ctx, Guard},
};

pub struct LoweredPredicate {
    pub predicate: pod2::middleware::CustomPredicate,
    pub wildcard_names: Vec<String>,
}

/// Internal lowering context that accumulates statements and wildcard names.
/// The first three names are public wildcards (Prev, Action, Next); witnesses
/// are appended as private args.
struct LowerCtx<'a> {
    params: &'a Params,
    names: Vec<String>,
    stmts: Vec<StatementTmpl>,
    // Memoization for dynamic lookups to avoid duplicate DictContains emissions
    lookup_cache: HashMap<(String, String), Wildcard>,
}

impl<'a> LowerCtx<'a> {
    fn new(params: &'a Params) -> Self {
        Self {
            params,
            names: vec!["Prev".to_string(), "Action".to_string(), "Next".to_string()],
            stmts: Vec::new(),
            lookup_cache: HashMap::new(),
        }
    }
    fn wildcard(&mut self, name: &str) -> Wildcard {
        let idx = if let Some(pos) = self.names.iter().position(|n| n == name) {
            pos
        } else {
            let idx = self.names.len();
            self.names.push(name.to_string());
            idx
        };
        Wildcard::new(name.to_string(), idx)
    }
    fn ensure_witness(&mut self, base: &str) -> Wildcard {
        // create a fresh witness name like base_N
        let mut i = 1usize;
        loop {
            let cand = format!("{base}_{i}");
            if !self.names.iter().any(|n| n == &cand) {
                let idx = self.names.len();
                self.names.push(cand.clone());
                return Wildcard::new(cand, idx);
            }
            i += 1;
        }
    }
    fn add_stmt(&mut self, pred: NativePredicate, args: Vec<StatementTmplArg>) {
        self.stmts.push(StatementTmpl {
            pred: Predicate::Native(pred),
            args,
        });
    }
}

/// Intermediate representation of a statement argument before converting into
/// `StatementTmplArg`.
#[derive(Clone, Debug)]
enum Arg {
    Ak(Wildcard, Key),
    Lit(Value),
    Wc(Wildcard),
}

impl From<Arg> for StatementTmplArg {
    fn from(a: Arg) -> Self {
        match a {
            Arg::Ak(w, k) => StatementTmplArg::AnchoredKey(w, k),
            Arg::Lit(v) => StatementTmplArg::Literal(v),
            Arg::Wc(w) => StatementTmplArg::Wildcard(w),
        }
    }
}

fn repr_expr(e: &Expr) -> String {
    match e {
        Expr::Var(VarKind::Prev) => "Prev".to_string(),
        Expr::Var(VarKind::Action) => "Action".to_string(),
        Expr::Var(VarKind::Next) => "Next".to_string(),
        Expr::Var(VarKind::Witness(n)) => format!("W({})", n),
        Expr::Const(v) => format!("Lit({v:?})"),
        Expr::Field(base, key) => format!("{}.{}", repr_expr(base), key),
        Expr::EmptyDict => "{}".to_string(),
        Expr::EmptySet => "#[]".to_string(),
        Expr::ContainerLookup { dict, key } => {
            format!("Lookup({}, {})", repr_expr(dict), repr_expr(key))
        }
        // Other expressions collapsed to informative strings for memoization keys
        Expr::Add(a, b) => format!("Add({}, {})", repr_expr(a), repr_expr(b)),
        Expr::Mul(a, b) => format!("Mul({}, {})", repr_expr(a), repr_expr(b)),
        Expr::Max(a, b) => format!("Max({}, {})", repr_expr(a), repr_expr(b)),
        Expr::SetInsert { set, elem } => format!("SetIns({}, {})", repr_expr(set), repr_expr(elem)),
        Expr::SetDelete { set, elem } => format!("SetDel({}, {})", repr_expr(set), repr_expr(elem)),
        Expr::DictInsert { dict, key, val } => {
            format!("DictIns({}, {}, {})", repr_expr(dict), key, repr_expr(val))
        }
        Expr::DictUpdate { dict, key, val } => {
            format!("DictUpd({}, {}, {})", repr_expr(dict), key, repr_expr(val))
        }
        Expr::DictDelete { dict, key } => format!("DictDel({}, {})", repr_expr(dict), key),
        Expr::ContainerInsert { dict, key, val } => format!(
            "ContIns({}, {}, {})",
            repr_expr(dict),
            repr_expr(key),
            repr_expr(val)
        ),
        Expr::ContainerUpdate { dict, key, val } => format!(
            "ContUpd({}, {}, {})",
            repr_expr(dict),
            repr_expr(key),
            repr_expr(val)
        ),
        Expr::ContainerDelete { dict, key } => {
            format!("ContDel({}, {})", repr_expr(dict), repr_expr(key))
        }
        Expr::DictUpdateChain { base, sets } => {
            let parts: Vec<String> = sets
                .iter()
                .map(|(k, v)| format!("{}.{}={},", repr_expr(base), k, repr_expr(v)))
                .collect();
            format!("Chain({})", parts.join(""))
        }
        Expr::ArrayUpdate { array, index, val } => {
            format!(
                "ArrUpd({}, {}, {})",
                repr_expr(array),
                repr_expr(index),
                repr_expr(val)
            )
        }
    }
}

fn lower_as_arg(ctx: &mut LowerCtx, e: &Expr) -> Arg {
    match e {
        Expr::Var(VarKind::Prev) => Arg::Wc(ctx.wildcard("Prev")),
        Expr::Var(VarKind::Action) => Arg::Wc(ctx.wildcard("Action")),
        Expr::Var(VarKind::Next) => Arg::Wc(ctx.wildcard("Next")),
        Expr::Var(VarKind::Witness(name)) => Arg::Wc(ctx.wildcard(name)),
        Expr::Const(v) => Arg::Lit(v.clone()),
        Expr::EmptyDict => {
            let d = pod2::middleware::containers::Dictionary::new(
                ctx.params.max_depth_mt_containers,
                Default::default(),
            )
            .expect("empty dict creation should not fail");
            Arg::Lit(Value::from(d))
        }
        Expr::EmptySet => {
            let s = pod2::middleware::containers::Set::new(
                ctx.params.max_depth_mt_containers,
                Default::default(),
            )
            .expect("empty set creation should not fail");
            Arg::Lit(Value::from(s))
        }
        Expr::Field(base, key) => {
            // Field access becomes an AnchoredKey in Podlang: Root["key"]. The root must
            // resolve to a wildcard referencing a dictionary-like value (Prev/Action/Next or upd_N).
            let root_arg = lower_as_arg(ctx, base);
            let wc = match root_arg {
                Arg::Wc(w) => w,
                // If field of non-wildcard literal, not meaningful; for prototype, disallow
                Arg::Ak(w, _) => w,
                Arg::Lit(_) => panic!("Cannot take field of literal in prototype"),
            };
            Arg::Ak(wc, Key::from(key.clone()))
        }
        Expr::ContainerLookup { dict, key } => {
            // Dynamic read: bind a witness via DictContains(dict, key, out)
            // De-duplicate identical lookups to reduce witnesses and statements.
            let k = (repr_expr(dict), repr_expr(key));
            if let Some(w) = ctx.lookup_cache.get(&k) {
                return Arg::Wc(w.clone());
            }
            let out = ctx.ensure_witness("lk");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            let key_arg: StatementTmplArg = lower_as_arg(ctx, key).into();
            ctx.add_stmt(
                NativePredicate::DictContains,
                vec![dict_arg, key_arg, StatementTmplArg::Wildcard(out.clone())],
            );
            ctx.lookup_cache.insert(k, out.clone());
            Arg::Wc(out)
        }
        // For ops, produce a witness and a statement
        Expr::Add(a, b) => {
            let out = ctx.ensure_witness("sum");
            let a_arg: StatementTmplArg = lower_as_arg(ctx, a).into();
            let b_arg: StatementTmplArg = lower_as_arg(ctx, b).into();
            ctx.add_stmt(
                NativePredicate::SumOf,
                vec![StatementTmplArg::Wildcard(out.clone()), a_arg, b_arg],
            );
            Arg::Wc(out)
        }
        Expr::Mul(a, b) => {
            let out = ctx.ensure_witness("prod");
            let a_arg: StatementTmplArg = lower_as_arg(ctx, a).into();
            let b_arg: StatementTmplArg = lower_as_arg(ctx, b).into();
            ctx.add_stmt(
                NativePredicate::ProductOf,
                vec![StatementTmplArg::Wildcard(out.clone()), a_arg, b_arg],
            );
            Arg::Wc(out)
        }
        Expr::Max(a, b) => {
            let out = ctx.ensure_witness("max");
            let a_arg: StatementTmplArg = lower_as_arg(ctx, a).into();
            let b_arg: StatementTmplArg = lower_as_arg(ctx, b).into();
            ctx.add_stmt(
                NativePredicate::MaxOf,
                vec![StatementTmplArg::Wildcard(out.clone()), a_arg, b_arg],
            );
            Arg::Wc(out)
        }
        Expr::SetInsert { set, elem } => {
            let out = ctx.ensure_witness("set");
            let set_arg: StatementTmplArg = lower_as_arg(ctx, set).into();
            let elem_arg: StatementTmplArg = lower_as_arg(ctx, elem).into();
            ctx.add_stmt(
                NativePredicate::SetInsert,
                vec![StatementTmplArg::Wildcard(out.clone()), set_arg, elem_arg],
            );
            Arg::Wc(out)
        }
        Expr::SetDelete { set, elem } => {
            let out = ctx.ensure_witness("set");
            let set_arg: StatementTmplArg = lower_as_arg(ctx, set).into();
            let elem_arg: StatementTmplArg = lower_as_arg(ctx, elem).into();
            ctx.add_stmt(
                NativePredicate::SetDelete,
                vec![StatementTmplArg::Wildcard(out.clone()), set_arg, elem_arg],
            );
            Arg::Wc(out)
        }
        Expr::DictInsert { dict, key, val } => {
            let out = ctx.ensure_witness("upd");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            let val_arg: StatementTmplArg = lower_as_arg(ctx, val).into();
            ctx.add_stmt(
                NativePredicate::DictInsert,
                vec![
                    StatementTmplArg::Wildcard(out.clone()),
                    dict_arg,
                    StatementTmplArg::Literal(Value::from(key.clone())),
                    val_arg,
                ],
            );
            Arg::Wc(out)
        }
        Expr::DictUpdate { dict, key, val } => {
            let out = ctx.ensure_witness("upd");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            let val_arg: StatementTmplArg = lower_as_arg(ctx, val).into();
            ctx.add_stmt(
                NativePredicate::DictUpdate,
                vec![
                    StatementTmplArg::Wildcard(out.clone()),
                    dict_arg,
                    StatementTmplArg::Literal(Value::from(key.clone())),
                    val_arg,
                ],
            );
            Arg::Wc(out)
        }
        Expr::DictDelete { dict, key } => {
            let out = ctx.ensure_witness("upd");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            ctx.add_stmt(
                NativePredicate::DictDelete,
                vec![
                    StatementTmplArg::Wildcard(out.clone()),
                    dict_arg,
                    StatementTmplArg::Literal(Value::from(key.clone())),
                ],
            );
            Arg::Wc(out)
        }
        Expr::ContainerInsert { dict, key, val } => {
            let out = ctx.ensure_witness("upd");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            let key_arg: StatementTmplArg = lower_as_arg(ctx, key).into();
            let val_arg: StatementTmplArg = lower_as_arg(ctx, val).into();
            ctx.add_stmt(
                NativePredicate::ContainerInsert,
                vec![
                    StatementTmplArg::Wildcard(out.clone()),
                    dict_arg,
                    key_arg,
                    val_arg,
                ],
            );
            Arg::Wc(out)
        }
        Expr::ContainerUpdate { dict, key, val } => {
            let out = ctx.ensure_witness("upd");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            let key_arg: StatementTmplArg = lower_as_arg(ctx, key).into();
            let val_arg: StatementTmplArg = lower_as_arg(ctx, val).into();
            ctx.add_stmt(
                NativePredicate::ContainerUpdate,
                vec![
                    StatementTmplArg::Wildcard(out.clone()),
                    dict_arg,
                    key_arg,
                    val_arg,
                ],
            );
            Arg::Wc(out)
        }
        Expr::ContainerDelete { dict, key } => {
            let out = ctx.ensure_witness("upd");
            let dict_arg: StatementTmplArg = lower_as_arg(ctx, dict).into();
            let key_arg: StatementTmplArg = lower_as_arg(ctx, key).into();
            ctx.add_stmt(
                NativePredicate::ContainerDelete,
                vec![StatementTmplArg::Wildcard(out.clone()), dict_arg, key_arg],
            );
            Arg::Wc(out)
        }
        Expr::ArrayUpdate { array, index, val } => {
            let out = ctx.ensure_witness("arr");
            let arr_arg: StatementTmplArg = lower_as_arg(ctx, array).into();
            let idx_arg: StatementTmplArg = lower_as_arg(ctx, index).into();
            let val_arg: StatementTmplArg = lower_as_arg(ctx, val).into();
            ctx.add_stmt(
                NativePredicate::ArrayUpdate,
                vec![
                    StatementTmplArg::Wildcard(out.clone()),
                    arr_arg,
                    idx_arg,
                    val_arg,
                ],
            );
            Arg::Wc(out)
        }
        Expr::DictUpdateChain { base, sets } => {
            // Chain DictUpdate over the sets in order. If the base is EmptyDict,
            // prefer DictInsert to construct a fresh object.
            let mut dict_term: Arg = lower_as_arg(ctx, base);
            let mut last_out: Option<Wildcard> = None;
            let base_is_empty = matches!(**base, Expr::EmptyDict);
            for (key, val_expr) in sets.iter() {
                let val_arg: StatementTmplArg = lower_as_arg(ctx, val_expr).into();
                let out = ctx.ensure_witness("upd");
                let pred = if base_is_empty {
                    NativePredicate::DictInsert
                } else {
                    NativePredicate::DictUpdate
                };
                ctx.add_stmt(
                    pred,
                    vec![
                        StatementTmplArg::Wildcard(out.clone()),
                        dict_term.clone().into(),
                        // Keys in DictUpdate/Insert are string literals in Podlang.
                        StatementTmplArg::Literal(Value::from(key.clone())),
                        val_arg,
                    ],
                );
                dict_term = Arg::Wc(out.clone());
                last_out = Some(out);
            }
            Arg::Wc(last_out.expect("update must have at least one set"))
        }
    }
}

fn lower_guards(ctx: &mut LowerCtx, guards: &[Guard]) {
    for g in guards {
        match g {
            Guard::Native(pred, args) => {
                let lowered: Vec<StatementTmplArg> = args
                    .iter()
                    .map(|o| StatementTmplArg::from(lower_as_arg(ctx, o.expr())))
                    .collect();
                ctx.add_stmt(*pred, lowered);
            }
        }
    }
}

pub fn build_predicate<F>(params: &Params, name: &str, f: F) -> LoweredPredicate
where
    F: FnOnce(&mut Ctx) -> Obj,
{
    let mut ctx = Ctx::new();
    let next_expr = f(&mut ctx);

    let mut lctx = LowerCtx::new(params);

    // Lower guards first (ordering not semantically significant for AND)
    lower_guards(&mut lctx, &ctx.guards);
    // Lower reducer body, tying to Next
    let result_arg = lower_as_arg(&mut lctx, next_expr.expr());
    let next_wc = lctx.wildcard("Next");
    lctx.add_stmt(
        NativePredicate::Equal,
        vec![StatementTmplArg::Wildcard(next_wc), result_arg.into()],
    );

    // Build CustomPredicate with public args (Prev, Action, Next) and private witnesses
    let pred = pod2::middleware::CustomPredicate::and(
        params,
        name.to_string(),
        lctx.stmts.clone(),
        3, // Prev, Action, Next
        lctx.names.clone(),
    )
    .expect("valid custom predicate");

    LoweredPredicate {
        predicate: pred,
        wildcard_names: lctx.names,
    }
}

// build_predicate_in_batch removed in simplified API
