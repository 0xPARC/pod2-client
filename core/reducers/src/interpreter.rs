use pod2::middleware::{containers::Dictionary as MwDict, Key, NativePredicate, TypedValue, Value};

use crate::{
    ast::{Expr, VarKind},
    builder::Obj,
    error::{Error, Result},
    guards::Guard,
    util::{as_array, as_dict, as_set, int_from_value},
};

struct EvalEnv<'a> {
    prev: &'a MwDict,
    action: &'a MwDict,
}

fn eval_expr(expr: &Expr, env: &EvalEnv) -> Result<Value> {
    match expr {
        Expr::Var(VarKind::Prev) => Ok(Value::from(env.prev.clone())),
        Expr::Var(VarKind::Action) => Ok(Value::from(env.action.clone())),
        Expr::Var(VarKind::Next) => Err(Error::Msg("Next is not evaluable in expression".into())),
        Expr::Var(VarKind::Witness(n)) => Err(Error::Unbound(n.clone())),
        Expr::Const(v) => Ok(v.clone()),
        Expr::Field(base, key) => {
            let base_v = eval_expr(base, env)?;
            match base_v.typed() {
                TypedValue::Dictionary(d) => Ok(d.get(&Key::from(key.clone()))?.clone()),
                other => Err(Error::Type(format!("Field base not a dictionary: {other}"))),
            }
        }
        Expr::Add(a, b) => {
            let va = eval_expr(a, env)?;
            let vb = eval_expr(b, env)?;
            Ok(Value::from(int_from_value(&va)? + int_from_value(&vb)?))
        }
        Expr::Mul(a, b) => {
            let va = eval_expr(a, env)?;
            let vb = eval_expr(b, env)?;
            Ok(Value::from(int_from_value(&va)? * int_from_value(&vb)?))
        }
        Expr::Max(a, b) => {
            let va = eval_expr(a, env)?;
            let vb = eval_expr(b, env)?;
            Ok(Value::from(int_from_value(&va)?.max(int_from_value(&vb)?)))
        }
        Expr::SetInsert { set, elem } => {
            let vs = eval_expr(set, env)?;
            let e = eval_expr(elem, env)?;
            let mut s = as_set(&vs)?;
            let _ = s.insert(&e)?;
            Ok(Value::from(s))
        }
        Expr::SetDelete { set, elem } => {
            let vs = eval_expr(set, env)?;
            let e = eval_expr(elem, env)?;
            let mut s = as_set(&vs)?;
            let _ = s.delete(&e)?;
            Ok(Value::from(s))
        }
        Expr::DictInsert { dict, key, val } => {
            let vd = eval_expr(dict, env)?;
            let v = eval_expr(val, env)?;
            let mut d = as_dict(&vd)?;
            let _ = d.insert(&Key::from(key.clone()), &v)?;
            Ok(Value::from(d))
        }
        Expr::DictUpdate { dict, key, val } => {
            let vd = eval_expr(dict, env)?;
            let v = eval_expr(val, env)?;
            let mut d = as_dict(&vd)?;
            let _ = d.update(&Key::from(key.clone()), &v)?;
            Ok(Value::from(d))
        }
        Expr::DictDelete { dict, key } => {
            let vd = eval_expr(dict, env)?;
            let mut d = as_dict(&vd)?;
            let _ = d.delete(&Key::from(key.clone()))?;
            Ok(Value::from(d))
        }
        Expr::DictUpdateChain { base, sets } => {
            let mut cur = as_dict(&eval_expr(base, env)?)?;
            for (k, ve) in sets {
                let v = eval_expr(ve, env)?;
                let _ = cur.update(&Key::from(k.clone()), &v)?;
            }
            Ok(Value::from(cur))
        }
        Expr::ArrayUpdate { array, index, val } => {
            let va = eval_expr(array, env)?;
            let idx = int_from_value(&eval_expr(index, env)?)? as usize;
            let v = eval_expr(val, env)?;
            let mut a = as_array(&va)?;
            let _ = a.update(idx, &v)?;
            Ok(Value::from(a))
        }
    }
}

fn check_guard(pred: NativePredicate, args: &[Obj], env: &EvalEnv) -> Result<bool> {
    let mut vals = Vec::with_capacity(args.len());
    for o in args {
        vals.push(eval_expr(o.expr(), env)?);
    }
    use NativePredicate as NP;
    let ok = match pred {
        NP::Equal => vals[0].raw() == vals[1].raw(),
        NP::NotEqual => vals[0].raw() != vals[1].raw(),
        NP::Lt => int_from_value(&vals[0])? < int_from_value(&vals[1])?,
        NP::LtEq => int_from_value(&vals[0])? <= int_from_value(&vals[1])?,
        NP::Gt => int_from_value(&vals[0])? > int_from_value(&vals[1])?,
        NP::GtEq => int_from_value(&vals[0])? >= int_from_value(&vals[1])?,
        NP::DictContains => {
            let d = as_dict(&vals[0])?;
            let key = Key::try_from(vals[1].typed())
                .map_err(|_| Error::Type("Key must be string".into()))?;
            let v = d.get(&key).ok().cloned();
            if let Some(v0) = v {
                v0.raw() == vals[2].raw()
            } else {
                false
            }
        }
        NP::DictNotContains => {
            let d = as_dict(&vals[0])?;
            let key = Key::try_from(vals[1].typed())
                .map_err(|_| Error::Type("Key must be string".into()))?;
            d.get(&key).is_err()
        }
        NP::SetContains => {
            let s = as_set(&vals[0])?;
            s.set().contains(&vals[1])
        }
        NP::SetNotContains => {
            let s = as_set(&vals[0])?;
            !s.set().contains(&vals[1])
        }
        NP::ArrayContains => {
            let a = as_array(&vals[0])?;
            let idx = int_from_value(&vals[1])? as usize;
            a.get(idx)
                .ok()
                .map(|v| v.raw() == vals[2].raw())
                .unwrap_or(false)
        }
        // For other predicates used as guards, default to false to be safe
        _ => false,
    };
    Ok(ok)
}

pub fn exec_reducer(
    prev: &MwDict,
    action: &MwDict,
    expr: &Expr,
    guards: &[Guard],
) -> Result<MwDict> {
    let env = EvalEnv { prev, action };
    for g in guards {
        match g {
            Guard::Native(pred, args) => {
                if !check_guard(*pred, args, &env)? {
                    return Err(Error::Msg(format!("Guard failed: {pred:?}")));
                }
            }
            Guard::CustomByName(name, _args) => {
                return Err(Error::Msg(format!(
                    "exec mode does not evaluate custom predicate calls: {name}"
                )));
            }
            Guard::CustomRef(_cpr, _args) => {
                return Err(Error::Msg(
                    "exec mode does not evaluate external custom predicate calls".into(),
                ));
            }
        }
    }
    let out = eval_expr(expr, &env)?;
    match out.typed() {
        TypedValue::Dictionary(d) => Ok(d.clone()),
        other => Err(Error::Type(format!(
            "Reducer did not return a dictionary: {other}"
        ))),
    }
}
