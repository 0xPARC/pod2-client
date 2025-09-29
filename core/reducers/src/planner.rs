use std::collections::HashMap;

use pod2::{
    frontend::Operation as FOperation,
    middleware::{
        CustomPredicateBatch, CustomPredicateRef, Key, NativeOperation, NativePredicate,
        OperationAux, OperationType, Params, Predicate, Statement, StatementTmplArg, TypedValue,
        Value,
    },
};

use crate::{
    error::{Error, Result},
    util::{as_array, as_dict, as_set, int_from_value},
};

fn value_of_arg(arg: &StatementTmplArg, env: &HashMap<String, Value>) -> Result<Value> {
    match arg {
        StatementTmplArg::Literal(v) => Ok(v.clone()),
        StatementTmplArg::Wildcard(w) => env
            .get(&w.name)
            .cloned()
            .ok_or_else(|| Error::Unbound(w.name.clone())),
        StatementTmplArg::AnchoredKey(w, key) => {
            let root = env
                .get(&w.name)
                .ok_or_else(|| Error::Unbound(w.name.clone()))?;
            match root.typed() {
                TypedValue::Dictionary(d) => Ok(d.get(key).map_err(Error::from)?.clone()),
                other => Err(Error::Type(format!(
                    "AnchoredKey root '{}' not a Dictionary (got {})",
                    w.name, other
                ))),
            }
        }
        StatementTmplArg::None => Err(Error::Msg("Unexpected None arg".to_string())),
    }
}

/// Given a lowered predicate and concrete bindings for its wildcards
/// (including Prev, Action, Next and any witnesses), produce a list of
/// middleware Operations that mirror the native predicates, with OperationAux
/// left as placeholders where external proofs/signatures are required.
pub fn plan_operations(
    pred: &pod2::middleware::CustomPredicate,
    _params: &Params,
    bindings: &HashMap<String, Value>,
) -> Result<Vec<FOperation>> {
    let mut env = bindings.clone();
    let mut ops: Vec<FOperation> = Vec::new();

    for st in pred.statements() {
        match st.pred() {
            Predicate::Native(np) => {
                match np {
                    NativePredicate::Equal => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::eq(a, b));
                    }
                    NativePredicate::NotEqual => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::ne(a, b));
                    }
                    NativePredicate::Lt => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::lt(a, b));
                    }
                    NativePredicate::LtEq => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::lt_eq(a, b));
                    }
                    NativePredicate::Gt => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::gt(a, b));
                    }
                    NativePredicate::GtEq => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::gt_eq(a, b));
                    }
                    NativePredicate::Contains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        let val = value_of_arg(&st.args[2], &env)?;
                        ops.push(FOperation::dict_contains(root, key, val));
                    }
                    NativePredicate::NotContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::dict_not_contains(root, key));
                    }
                    NativePredicate::DictContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        let val = value_of_arg(&st.args[2], &env)?;
                        ops.push(FOperation::dict_contains(root, key, val));
                    }
                    NativePredicate::DictNotContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::dict_not_contains(root, key));
                    }
                    NativePredicate::ArrayContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        let val = value_of_arg(&st.args[2], &env)?;
                        ops.push(FOperation::array_contains(root, key, val));
                    }
                    NativePredicate::SetContains => {
                        let s = value_of_arg(&st.args[0], &env)?;
                        let x = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::set_contains(s, x));
                    }
                    NativePredicate::SetNotContains => {
                        let s = value_of_arg(&st.args[0], &env)?;
                        let x = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::set_not_contains(s, x));
                    }
                    NativePredicate::SumOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => return Err(Error::Msg("SumOf first arg must be Wildcard".into())),
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let sum = int_from_value(&a)? + int_from_value(&b)?;
                        let out_val = Value::from(sum);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::sum_of(out_val, a, b));
                    }
                    NativePredicate::ProductOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ProductOf first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let prod = int_from_value(&a)? * int_from_value(&b)?;
                        let out_val = Value::from(prod);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::product_of(out_val, a, b));
                    }
                    NativePredicate::MaxOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => return Err(Error::Msg("MaxOf first arg must be Wildcard".into())),
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let max = int_from_value(&a)?.max(int_from_value(&b)?);
                        let out_val = Value::from(max);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::max_of(out_val, a, b));
                    }
                    NativePredicate::HashOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg("HashOf first arg must be Wildcard".into()))
                            }
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        // Defer computing hash; use op to check
                        let out_val = env.get(&out_wc).cloned().unwrap_or_else(|| Value::from(0));
                        ops.push(FOperation::hash_of(out_val, a, b));
                    }
                    NativePredicate::PublicKeyOf => {
                        let sk = value_of_arg(&st.args[0], &env)?;
                        let pk = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::public_key_of(sk, pk));
                    }
                    NativePredicate::SignedBy => {
                        let msg = value_of_arg(&st.args[0], &env)?;
                        let pk = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation(
                            OperationType::Native(NativeOperation::SignedBy),
                            vec![msg.into(), pk.into()],
                            OperationAux::None,
                        ));
                    }
                    NativePredicate::DictInsert => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "DictInsert first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.insert(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_insert(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::DictUpdate => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "DictUpdate first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.update(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_update(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::DictDelete => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "DictDelete first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.delete(&key).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_delete(
                            out_val,
                            dict,
                            Value::from(key_name),
                        ));
                    }
                    NativePredicate::SetInsert => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "SetInsert first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let set_v = value_of_arg(&st.args[1], &env)?;
                        let elem = value_of_arg(&st.args[2], &env)?;
                        let mut s = as_set(&set_v)?;
                        let _ = s.insert(&elem).map_err(Error::from)?;
                        let out_val = Value::from(s);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::set_insert(out_val, set_v, elem));
                    }
                    NativePredicate::SetDelete => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "SetDelete first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let set_v = value_of_arg(&st.args[1], &env)?;
                        let elem = value_of_arg(&st.args[2], &env)?;
                        let mut s = as_set(&set_v)?;
                        let _ = s.delete(&elem).map_err(Error::from)?;
                        let out_val = Value::from(s);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::set_delete(out_val, set_v, elem));
                    }
                    NativePredicate::ArrayUpdate => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ArrayUpdate first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let arr_v = value_of_arg(&st.args[1], &env)?;
                        let idx_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut a = as_array(&arr_v)?;
                        let idx = int_from_value(&idx_v)? as usize;
                        let _ = a.update(idx, &val).map_err(Error::from)?;
                        let out_val = Value::from(a);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::array_update(out_val, arr_v, idx_v, val));
                    }
                    NativePredicate::None | NativePredicate::False => {}
                    NativePredicate::ContainerInsert => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ContainerInsert first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.insert(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_insert(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::ContainerUpdate => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ContainerUpdate first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.update(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_update(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::ContainerDelete => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ContainerDelete first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.delete(&key).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_delete(
                            out_val,
                            dict,
                            Value::from(key_name),
                        ));
                    }
                }
            }
            Predicate::Custom(cpr) => {
                let arity = cpr.predicate().statements().len();
                ops.push(FOperation::custom(
                    cpr.clone(),
                    std::iter::repeat_n(pod2::middleware::Statement::None, arity),
                ));
            }
            Predicate::BatchSelf(_) | Predicate::Intro(_) => {
                // Standalone planner cannot resolve batch-local calls or intro; ignore here.
            }
        }
    }

    Ok(ops)
}

/// Plan operations for a predicate that may include custom predicate calls.
/// Requires the `batch` that contains the predicate so BatchSelf indices can
/// be resolved into `CustomPredicateRef`s. For custom calls, pushes a
/// placeholder `Operation::custom(...)` with `Statement::None` entries of the
/// correct arity.
pub fn plan_operations_in_batch(
    pred: &pod2::middleware::CustomPredicate,
    params: &Params,
    bindings: &HashMap<String, Value>,
    batch: &std::sync::Arc<CustomPredicateBatch>,
) -> Result<Vec<FOperation>> {
    let mut env = bindings.clone();
    let mut ops: Vec<FOperation> = Vec::new();

    for st in pred.statements() {
        match st.pred() {
            Predicate::Native(np) => {
                // Reuse native handling by temporarily matching on np as in `plan_operations`
                match np {
                    NativePredicate::Equal => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::eq(a, b));
                    }
                    NativePredicate::NotEqual => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::ne(a, b));
                    }
                    NativePredicate::Lt => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::lt(a, b));
                    }
                    NativePredicate::LtEq => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::lt_eq(a, b));
                    }
                    NativePredicate::Gt => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::gt(a, b));
                    }
                    NativePredicate::GtEq => {
                        let a = value_of_arg(&st.args[0], &env)?;
                        let b = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::gt_eq(a, b));
                    }
                    NativePredicate::Contains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        let val = value_of_arg(&st.args[2], &env)?;
                        ops.push(FOperation::dict_contains(root, key, val));
                    }
                    NativePredicate::NotContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::dict_not_contains(root, key));
                    }
                    NativePredicate::DictContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        let val = value_of_arg(&st.args[2], &env)?;
                        ops.push(FOperation::dict_contains(root, key, val));
                    }
                    NativePredicate::DictNotContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::dict_not_contains(root, key));
                    }
                    NativePredicate::ArrayContains => {
                        let root = value_of_arg(&st.args[0], &env)?;
                        let key = value_of_arg(&st.args[1], &env)?;
                        let val = value_of_arg(&st.args[2], &env)?;
                        ops.push(FOperation::array_contains(root, key, val));
                    }
                    NativePredicate::SetContains => {
                        let s = value_of_arg(&st.args[0], &env)?;
                        let x = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::set_contains(s, x));
                    }
                    NativePredicate::SetNotContains => {
                        let s = value_of_arg(&st.args[0], &env)?;
                        let x = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::set_not_contains(s, x));
                    }
                    NativePredicate::SumOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => return Err(Error::Msg("SumOf first arg must be Wildcard".into())),
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let sum = int_from_value(&a)? + int_from_value(&b)?;
                        let out_val = Value::from(sum);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::sum_of(out_val, a, b));
                    }
                    NativePredicate::ProductOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ProductOf first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let prod = int_from_value(&a)? * int_from_value(&b)?;
                        let out_val = Value::from(prod);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::product_of(out_val, a, b));
                    }
                    NativePredicate::MaxOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => return Err(Error::Msg("MaxOf first arg must be Wildcard".into())),
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let max = int_from_value(&a)?.max(int_from_value(&b)?);
                        let out_val = Value::from(max);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::max_of(out_val, a, b));
                    }
                    NativePredicate::HashOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg("HashOf first arg must be Wildcard".into()))
                            }
                        };
                        let a = value_of_arg(&st.args[1], &env)?;
                        let b = value_of_arg(&st.args[2], &env)?;
                        let out_val = env.get(&out_wc).cloned().unwrap_or_else(|| Value::from(0));
                        ops.push(FOperation::hash_of(out_val, a, b));
                    }
                    NativePredicate::PublicKeyOf => {
                        let sk = value_of_arg(&st.args[0], &env)?;
                        let pk = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation::public_key_of(sk, pk));
                    }
                    NativePredicate::SignedBy => {
                        let msg = value_of_arg(&st.args[0], &env)?;
                        let pk = value_of_arg(&st.args[1], &env)?;
                        ops.push(FOperation(
                            OperationType::Native(NativeOperation::SignedBy),
                            vec![msg.into(), pk.into()],
                            OperationAux::None,
                        ));
                    }
                    NativePredicate::DictInsert => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "DictInsert first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.insert(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_insert(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::DictUpdate => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "DictUpdate first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.update(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_update(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::DictDelete => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "DictDelete first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.delete(&key).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_delete(
                            out_val,
                            dict,
                            Value::from(key_name),
                        ));
                    }
                    NativePredicate::SetInsert => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "SetInsert first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let set_v = value_of_arg(&st.args[1], &env)?;
                        let elem = value_of_arg(&st.args[2], &env)?;
                        let mut s = as_set(&set_v)?;
                        let _ = s.insert(&elem).map_err(Error::from)?;
                        let out_val = Value::from(s);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::set_insert(out_val, set_v, elem));
                    }
                    NativePredicate::SetDelete => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "SetDelete first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let set_v = value_of_arg(&st.args[1], &env)?;
                        let elem = value_of_arg(&st.args[2], &env)?;
                        let mut s = as_set(&set_v)?;
                        let _ = s.delete(&elem).map_err(Error::from)?;
                        let out_val = Value::from(s);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::set_delete(out_val, set_v, elem));
                    }
                    NativePredicate::ArrayUpdate => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ArrayUpdate first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let arr_v = value_of_arg(&st.args[1], &env)?;
                        let idx_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut a = as_array(&arr_v)?;
                        let idx = int_from_value(&idx_v)? as usize;
                        let _ = a.update(idx, &val).map_err(Error::from)?;
                        let out_val = Value::from(a);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::array_update(out_val, arr_v, idx_v, val));
                    }
                    NativePredicate::None | NativePredicate::False => {}
                    NativePredicate::ContainerInsert => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ContainerInsert first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.insert(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_insert(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::ContainerUpdate => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ContainerUpdate first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.update(&key, &val).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_update(
                            out_val,
                            dict,
                            Value::from(key_name),
                            val,
                        ));
                    }
                    NativePredicate::ContainerDelete => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => {
                                return Err(Error::Msg(
                                    "ContainerDelete first arg must be Wildcard".into(),
                                ))
                            }
                        };
                        let dict = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let mut d = as_dict(&dict)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.delete(&key).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());
                        ops.push(FOperation::dict_delete(
                            out_val,
                            dict,
                            Value::from(key_name),
                        ));
                    }
                    NativePredicate::Gt | NativePredicate::GtEq => {}
                }
            }
            Predicate::BatchSelf(i) => {
                let cpr = CustomPredicateRef::new(batch.clone(), *i);
                let arity = batch.predicates()[*i].statements().len();
                let op = FOperation::custom(cpr, std::iter::repeat_n(Statement::None, arity));
                ops.push(op);
            }
            Predicate::Custom(cpr) => {
                let arity = cpr.predicate().statements().len();
                let op =
                    FOperation::custom(cpr.clone(), std::iter::repeat_n(Statement::None, arity));
                ops.push(op);
            }
            Predicate::Intro(_ir) => {
                // Placeholder: intros not planned yet.
            }
        }
    }

    Ok(ops)
}
