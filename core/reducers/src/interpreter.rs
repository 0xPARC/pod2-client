use std::collections::HashMap;

use pod2::{
    frontend::SignedDict,
    middleware::{
        containers::{Dictionary as MwDict, Set as MwSet},
        Key, NativePredicate, Params, TypedValue, Value,
    },
};

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
    bindings: HashMap<String, Value>,
}

#[derive(Default)]
pub struct ExecAux {
    pub signed_action: Option<SignedDict>,
}

fn eval_expr(expr: &Expr, env: &EvalEnv) -> Result<Value> {
    match expr {
        Expr::Var(VarKind::Prev) => Ok(Value::from(env.prev.clone())),
        Expr::Var(VarKind::Action) => Ok(Value::from(env.action.clone())),
        Expr::Var(VarKind::Next) => Err(Error::Msg("Next is not evaluable in expression".into())),
        Expr::Var(VarKind::Witness(n)) => env
            .bindings
            .get(n)
            .cloned()
            .ok_or_else(|| Error::Unbound(n.clone())),
        Expr::Const(v) => Ok(v.clone()),
        Expr::EmptyDict => {
            let d = MwDict::new(
                Params::default().max_depth_mt_containers,
                Default::default(),
            )
            .expect("empty dict creation should not fail");
            Ok(Value::from(d))
        }
        Expr::EmptySet => {
            let s = MwSet::new(
                Params::default().max_depth_mt_containers,
                Default::default(),
            )
            .expect("empty set creation should not fail");
            Ok(Value::from(s))
        }
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
        Expr::ContainerInsert { dict, key, val } => {
            let vd = eval_expr(dict, env)?;
            let vk = eval_expr(key, env)?;
            let vv = eval_expr(val, env)?;
            let mut d = as_dict(&vd)?;
            let k =
                Key::try_from(vk.typed()).map_err(|_| Error::Type("Key must be string".into()))?;
            let _ = d.insert(&k, &vv)?;
            Ok(Value::from(d))
        }
        Expr::ContainerLookup { dict, key } => {
            let vd = eval_expr(dict, env)?;
            let vk = eval_expr(key, env)?;
            let d = as_dict(&vd)?;
            let k =
                Key::try_from(vk.typed()).map_err(|_| Error::Type("Key must be string".into()))?;
            Ok(d.get(&k)?.clone())
        }
        Expr::ContainerUpdate { dict, key, val } => {
            let vd = eval_expr(dict, env)?;
            let vk = eval_expr(key, env)?;
            let vv = eval_expr(val, env)?;
            let mut d = as_dict(&vd)?;
            let k =
                Key::try_from(vk.typed()).map_err(|_| Error::Type("Key must be string".into()))?;
            let _ = d.update(&k, &vv)?;
            Ok(Value::from(d))
        }
        Expr::ContainerDelete { dict, key } => {
            let vd = eval_expr(dict, env)?;
            let vk = eval_expr(key, env)?;
            let mut d = as_dict(&vd)?;
            let k =
                Key::try_from(vk.typed()).map_err(|_| Error::Type("Key must be string".into()))?;
            let _ = d.delete(&k)?;
            Ok(Value::from(d))
        }
        Expr::DictUpdateChain { base, sets } => {
            let mut cur = as_dict(&eval_expr(base, env)?)?;
            for (k, ve) in sets {
                let v = eval_expr(ve, env)?;
                let key = Key::from(k.clone());
                if cur.get(&key).is_ok() {
                    let _ = cur.update(&key, &v)?;
                } else {
                    let _ = cur.insert(&key, &v)?;
                }
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

fn check_guard(pred: NativePredicate, args: &[Obj], env: &mut EvalEnv) -> Result<bool> {
    // For most predicates we evaluate all argument expressions up front.
    // DictContains is special because the 3rd argument may be an unbound
    // witness that we want to bind here, so we handle it in its match arm
    // without pre-evaluating args to avoid an unbound error.
    let mut vals: Vec<Value> = Vec::with_capacity(args.len());
    use NativePredicate as NP;
    if pred != NP::DictContains {
        for a in args {
            vals.push(eval_expr(a.expr(), env)?);
        }
    }
    let ok = match pred {
        NP::Equal => vals[0].raw() == vals[1].raw(),
        NP::NotEqual => vals[0].raw() != vals[1].raw(),
        NP::Lt => int_from_value(&vals[0])? < int_from_value(&vals[1])?,
        NP::LtEq => int_from_value(&vals[0])? <= int_from_value(&vals[1])?,
        NP::Gt => int_from_value(&vals[0])? > int_from_value(&vals[1])?,
        NP::GtEq => int_from_value(&vals[0])? >= int_from_value(&vals[1])?,
        NP::DictContains => {
            // Special-case: if third arg is a witness, bind it from the dict
            let dict_expr = args[0].expr();
            let key_expr = args[1].expr();
            let dict_v = eval_expr(dict_expr, env)?;
            let key_v = eval_expr(key_expr, env)?;
            let d = as_dict(&dict_v)?;
            let key = Key::try_from(key_v.typed())
                .map_err(|_| Error::Type("Key must be string".into()))?;
            let found = d.get(&key).ok().cloned();
            match args[2].expr() {
                Expr::Var(VarKind::Witness(name)) => {
                    if let Some(v) = found {
                        env.bindings.insert(name.clone(), v.clone());
                        true
                    } else {
                        false
                    }
                }
                other => {
                    let expect = eval_expr(other, env)?;
                    if let Some(v0) = found {
                        v0.raw() == expect.raw()
                    } else {
                        false
                    }
                }
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
    let mut env = EvalEnv {
        prev,
        action,
        bindings: HashMap::new(),
    };
    for g in guards {
        let Guard::Native(pred, args) = g;
        if !check_guard(*pred, args, &mut env)? {
            return Err(Error::Msg(format!("Guard failed: {pred:?}")));
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

pub fn exec_reducer_with_aux(
    prev: &MwDict,
    action: &MwDict,
    expr: &Expr,
    guards: &[Guard],
    aux: &ExecAux,
) -> Result<MwDict> {
    let mut env = EvalEnv {
        prev,
        action,
        bindings: HashMap::new(),
    };
    for g in guards {
        let Guard::Native(pred, args) = g;
        if *pred == NativePredicate::SignedBy {
            // Expect (dict, public_key) and a matching SignedDict in aux
            if let Some(sd) = &aux.signed_action {
                let dict_v = eval_expr(args[0].expr(), &env)?;
                let pk_v = eval_expr(args[1].expr(), &env)?;
                let ok_pk = match pk_v.typed() {
                    TypedValue::PublicKey(pk) => *pk == sd.public_key,
                    _ => false,
                };
                let ok_dict = match dict_v.typed() {
                    TypedValue::Dictionary(d) => d.commitment() == sd.dict.commitment(),
                    _ => false,
                };
                if !(ok_pk && ok_dict && sd.verify().is_ok()) {
                    return Err(Error::Msg("Guard failed: SignedBy".into()));
                }
            } else {
                return Err(Error::Msg(
                    "Guard failed: SignedBy (no signed action)".into(),
                ));
            }
        } else if !check_guard(*pred, args, &mut env)? {
            return Err(Error::Msg(format!("Guard failed: {pred:?}")));
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
