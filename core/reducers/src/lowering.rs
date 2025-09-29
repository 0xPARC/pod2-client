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
}

impl<'a> LowerCtx<'a> {
    fn new(params: &'a Params) -> Self {
        Self {
            params,
            names: vec!["Prev".to_string(), "Action".to_string(), "Next".to_string()],
            stmts: Vec::new(),
        }
    }
    fn wildcard(&self, name: &str) -> Wildcard {
        let idx = self
            .names
            .iter()
            .position(|n| n == name)
            .expect("wildcard must exist");
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

fn lower_as_arg(ctx: &mut LowerCtx, e: &Expr) -> Arg {
    match e {
        Expr::Var(VarKind::Prev) => Arg::Wc(ctx.wildcard("Prev")),
        Expr::Var(VarKind::Action) => Arg::Wc(ctx.wildcard("Action")),
        Expr::Var(VarKind::Next) => Arg::Wc(ctx.wildcard("Next")),
        Expr::Var(VarKind::Witness(name)) => Arg::Wc(ctx.wildcard(name)),
        Expr::Const(v) => Arg::Lit(v.clone()),
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
            // Chain DictUpdate over the sets in order
            let mut dict_term: Arg = lower_as_arg(ctx, base);
            let mut last_out: Option<Wildcard> = None;
            for (key, val_expr) in sets.iter() {
                let val_arg: StatementTmplArg = lower_as_arg(ctx, val_expr).into();
                let out = ctx.ensure_witness("upd");
                ctx.add_stmt(
                    NativePredicate::DictUpdate,
                    vec![
                        StatementTmplArg::Wildcard(out.clone()),
                        dict_term.clone().into(),
                        // Keys in DictUpdate are string literals in Podlang; the frontend
                        // desugars DictUpdate → ContainerUpdate using this literal key.
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

fn lower_guards(
    ctx: &mut LowerCtx,
    guards: &[Guard],
    name_to_index: Option<&HashMap<String, usize>>,
) {
    for g in guards {
        match g {
            Guard::Native(pred, args) => {
                let lowered: Vec<StatementTmplArg> = args
                    .iter()
                    .map(|o| StatementTmplArg::from(lower_as_arg(ctx, o.expr())))
                    .collect();
                ctx.add_stmt(*pred, lowered);
            }
            Guard::CustomByName(name, args) => {
                let Some(map) = name_to_index else {
                    panic!("CustomByName guard used without batch name resolver");
                };
                let idx = map
                    .get(name)
                    .copied()
                    .unwrap_or_else(|| panic!("Unknown custom predicate name: {name}"));
                let lowered: Vec<StatementTmplArg> = args
                    .iter()
                    .map(|o| StatementTmplArg::from(lower_as_arg(ctx, o.expr())))
                    .collect();
                ctx.stmts.push(StatementTmpl {
                    pred: Predicate::BatchSelf(idx),
                    args: lowered,
                });
            }
            Guard::CustomRef(cpr, args) => {
                let lowered: Vec<StatementTmplArg> = args
                    .iter()
                    .map(|o| StatementTmplArg::from(lower_as_arg(ctx, o.expr())))
                    .collect();
                ctx.stmts.push(StatementTmpl {
                    pred: Predicate::Custom(cpr.clone()),
                    args: lowered,
                });
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
    lower_guards(&mut lctx, &ctx.guards, None);
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

/// Build a predicate intended to be part of a batch, resolving any
/// `Guard::CustomByName` entries against the provided `name_to_index` map
/// using `Predicate::BatchSelf(index)`.
pub fn build_predicate_in_batch<F>(
    params: &Params,
    name: &str,
    f: F,
    name_to_index: &HashMap<String, usize>,
) -> pod2::middleware::CustomPredicate
where
    F: FnOnce(&mut Ctx) -> Obj,
{
    let mut ctx = Ctx::new();
    let next_expr = f(&mut ctx);
    let mut lctx = LowerCtx::new(params);
    lower_guards(&mut lctx, &ctx.guards, Some(name_to_index));
    let result_arg = lower_as_arg(&mut lctx, next_expr.expr());
    let next_wc = lctx.wildcard("Next");
    lctx.add_stmt(
        NativePredicate::Equal,
        vec![StatementTmplArg::Wildcard(next_wc), result_arg.into()],
    );
    pod2::middleware::CustomPredicate::and(
        params,
        name.to_string(),
        lctx.stmts.clone(),
        3,
        lctx.names.clone(),
    )
    .expect("valid custom predicate")
}
