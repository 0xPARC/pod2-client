use std::collections::HashMap;

use pod2::{
    frontend::{Operation as FOperation, SignedDict},
    middleware::{
        Key, NativeOperation, NativePredicate, OperationAux, OperationType, Params, Predicate,
        Statement as MwStatement, StatementTmplArg, TypedValue, Value,
    },
};

use crate::{
    error::{Error, Result},
    util::{as_array, as_dict, as_set, int_from_value},
};

fn op_arg_from_tmpl_arg(
    arg: &StatementTmplArg,
    env: &HashMap<String, Value>,
) -> Result<pod2::frontend::OperationArg> {
    match arg {
        StatementTmplArg::Literal(v) => Ok(pod2::frontend::OperationArg::from(v.clone())),
        StatementTmplArg::Wildcard(w) => {
            let v = env
                .get(&w.name)
                .cloned()
                .ok_or_else(|| Error::Unbound(w.name.clone()))?;
            Ok(pod2::frontend::OperationArg::from(v))
        }
        StatementTmplArg::AnchoredKey(wc, key) => {
            let root = env
                .get(&wc.name)
                .cloned()
                .ok_or_else(|| Error::Unbound(wc.name.clone()))?;
            let dict = as_dict(&root)?;
            let val = dict.get(key).map_err(Error::from)?.clone();
            Ok(pod2::frontend::OperationArg::from(MwStatement::Contains(
                Value::from(dict.clone()).into(),
                Value::from(key.name()).into(),
                val.into(),
            )))
        }
        StatementTmplArg::None => Err(Error::Msg("Unexpected None arg in template".into())),
    }
}

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
                        // Build args from template to preserve AnchoredKey refs
                        let a = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                        let b = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        ops.push(FOperation::eq(a, b));
                    }
                    NativePredicate::NotEqual => {
                        let a = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                        let b = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        ops.push(FOperation::ne(a, b));
                    }
                    NativePredicate::Lt => {
                        let a = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                        let b = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        ops.push(FOperation::lt(a, b));
                    }
                    NativePredicate::LtEq => {
                        let a = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                        let b = op_arg_from_tmpl_arg(&st.args[1], &env)?;
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
                        // If third arg is a wildcard: bind it from container[key]
                        if let StatementTmplArg::Wildcard(w) = &st.args[2] {
                            let root_v = value_of_arg(&st.args[0], &env)?;
                            let key_v = value_of_arg(&st.args[1], &env)?;
                            let val = match root_v.typed() {
                                TypedValue::Dictionary(d) => {
                                    let key_name = match key_v.typed() {
                                        TypedValue::String(s) => s.clone(),
                                        other => {
                                            return Err(Error::Type(format!(
                                                "Key must be string, got {other}"
                                            )))
                                        }
                                    };
                                    d.get(&Key::from(key_name)).map_err(Error::from)?.clone()
                                }
                                TypedValue::Set(_) => {
                                    // For sets, third arg equals element; bind to key_v to maintain shape
                                    key_v.clone()
                                }
                                TypedValue::Array(a) => {
                                    let idx = int_from_value(&key_v)? as usize;
                                    a.get(idx).map_err(Error::from)?.clone()
                                }
                                other => {
                                    return Err(Error::Type(format!(
                                        "Contains root must be container, got {other}"
                                    )))
                                }
                            };
                            env.insert(w.name.clone(), val.clone());
                            let a0 = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                            let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                            let a2 = pod2::frontend::OperationArg::from(val);
                            ops.push(FOperation::dict_contains(a0, a1, a2));
                        } else {
                            let a0 = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                            let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                            let a2 = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                            ops.push(FOperation::dict_contains(a0, a1, a2));
                        }
                    }
                    NativePredicate::NotContains => {
                        let a0 = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                        let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        ops.push(FOperation::dict_not_contains(a0, a1));
                    }
                    NativePredicate::DictContains => {
                        // If third arg is a wildcard, bind it by reading from dict[key]
                        if let StatementTmplArg::Wildcard(w) = &st.args[2] {
                            let dict_v = value_of_arg(&st.args[0], &env)?;
                            let key_v = value_of_arg(&st.args[1], &env)?;
                            let d = as_dict(&dict_v)?;
                            let key_name = match key_v.typed() {
                                TypedValue::String(s) => s.clone(),
                                other => {
                                    return Err(Error::Type(format!(
                                        "Key must be string, got {other}"
                                    )))
                                }
                            };
                            let val = d.get(&Key::from(key_name)).map_err(Error::from)?.clone();
                            env.insert(w.name.clone(), val.clone());
                            let a0 = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                            let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                            let a2 = pod2::frontend::OperationArg::from(val);
                            ops.push(FOperation::dict_contains(a0, a1, a2));
                        } else {
                            let a0 = op_arg_from_tmpl_arg(&st.args[0], &env)?;
                            let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                            let a2 = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                            ops.push(FOperation::dict_contains(a0, a1, a2));
                        }
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
                        let a_val = value_of_arg(&st.args[1], &env)?;
                        let b_val = value_of_arg(&st.args[2], &env)?;
                        let sum = int_from_value(&a_val)? + int_from_value(&b_val)?;
                        let out_val = Value::from(sum);
                        env.insert(out_wc.clone(), out_val.clone());

                        let a_op = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        let b_op = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                        ops.push(FOperation::sum_of(out_val, a_op, b_op));
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
                        let a_val = value_of_arg(&st.args[1], &env)?;
                        let b_val = value_of_arg(&st.args[2], &env)?;
                        let prod = int_from_value(&a_val)? * int_from_value(&b_val)?;
                        let out_val = Value::from(prod);
                        env.insert(out_wc.clone(), out_val.clone());

                        let a_op = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        let b_op = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                        ops.push(FOperation::product_of(out_val, a_op, b_op));
                    }
                    NativePredicate::MaxOf => {
                        let out_wc = match &st.args[0] {
                            StatementTmplArg::Wildcard(w) => w.name.clone(),
                            _ => return Err(Error::Msg("MaxOf first arg must be Wildcard".into())),
                        };
                        let a_val = value_of_arg(&st.args[1], &env)?;
                        let b_val = value_of_arg(&st.args[2], &env)?;
                        let max = int_from_value(&a_val)?.max(int_from_value(&b_val)?);
                        let out_val = Value::from(max);
                        env.insert(out_wc.clone(), out_val.clone());

                        let a_op = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        let b_op = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                        ops.push(FOperation::max_of(out_val, a_op, b_op));
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
                        // Compute new root value to bind wildcard
                        let dict_v = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val_v = value_of_arg(&st.args[3], &env)?;
                        let out_val = match dict_v.typed() {
                            TypedValue::Dictionary(_) => {
                                let mut d = as_dict(&dict_v)?;
                                let key_name = match key_v.typed() {
                                    TypedValue::String(s) => s.clone(),
                                    other => {
                                        return Err(Error::Type(format!(
                                            "Key must be string, got {other}"
                                        )))
                                    }
                                };
                                let key = Key::from(key_name);
                                let _ = d.insert(&key, &val_v).map_err(Error::from)?;
                                Value::from(d)
                            }
                            TypedValue::Set(_) => {
                                let mut s = as_set(&dict_v)?;
                                let _ = s.insert(&val_v).map_err(Error::from)?;
                                Value::from(s)
                            }
                            other => {
                                return Err(Error::Type(format!(
                                    "ContainerInsert root must be container, got {other}"
                                )))
                            }
                        };
                        env.insert(out_wc.clone(), out_val.clone());

                        // Build args from template for anchoring (Prev/Action fields)
                        let a0 = pod2::frontend::OperationArg::from(out_val);
                        let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        let a2 = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                        let a3 = op_arg_from_tmpl_arg(&st.args[3], &env)?;
                        ops.push(FOperation::dict_insert(a0, a1, a2, a3));
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
                        let dict_v = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let val_v = value_of_arg(&st.args[3], &env)?;
                        let mut d = as_dict(&dict_v)?;
                        let key_name = match key_v.typed() {
                            TypedValue::String(s) => s.clone(),
                            other => {
                                return Err(Error::Type(format!("Key must be string, got {other}")))
                            }
                        };
                        let key = Key::from(key_name.clone());
                        let _ = d.update(&key, &val_v).map_err(Error::from)?;
                        let out_val = Value::from(d);
                        env.insert(out_wc.clone(), out_val.clone());

                        let a0 = pod2::frontend::OperationArg::from(out_val);
                        let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        let a2 = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                        let a3 = op_arg_from_tmpl_arg(&st.args[3], &env)?;
                        ops.push(FOperation::dict_update(a0, a1, a2, a3));
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
                        let dict_v = value_of_arg(&st.args[1], &env)?;
                        let key_v = value_of_arg(&st.args[2], &env)?;
                        let out_val = match dict_v.typed() {
                            TypedValue::Dictionary(_) => {
                                let mut d = as_dict(&dict_v)?;
                                let key_name = match key_v.typed() {
                                    TypedValue::String(s) => s.clone(),
                                    other => {
                                        return Err(Error::Type(format!(
                                            "Key must be string, got {other}"
                                        )))
                                    }
                                };
                                let key = Key::from(key_name);
                                let _ = d.delete(&key).map_err(Error::from)?;
                                Value::from(d)
                            }
                            TypedValue::Set(_) => {
                                let mut s = as_set(&dict_v)?;
                                // For sets, 'key' carries element value
                                let _ = s.delete(&key_v).map_err(Error::from)?;
                                Value::from(s)
                            }
                            other => {
                                return Err(Error::Type(format!(
                                    "ContainerDelete root must be container, got {other}"
                                )))
                            }
                        };
                        env.insert(out_wc.clone(), out_val.clone());

                        let a0 = pod2::frontend::OperationArg::from(out_val);
                        let a1 = op_arg_from_tmpl_arg(&st.args[1], &env)?;
                        let a2 = op_arg_from_tmpl_arg(&st.args[2], &env)?;
                        ops.push(FOperation::dict_delete(a0, a1, a2));
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
            _ => {}
        }
    }

    Ok(ops)
}

/// Post-process planned operations to attach an actual signature to `SignedBy` operations.
///
/// Validates that each `SignedBy(msg, pk)` matches the provided `SignedDict`'s dictionary and
/// public key, then replaces the op with one that carries `OperationAux::Signature`.
pub fn attach_signature_to_ops(
    ops: Vec<FOperation>,
    signed: &SignedDict,
) -> Result<Vec<FOperation>> {
    use pod2::middleware::{NativeOperation::SignedBy, OperationType::Native};
    let mut out: Vec<FOperation> = Vec::with_capacity(ops.len());
    for op in ops.into_iter() {
        match (&op.0, &op.2) {
            (Native(SignedBy), OperationAux::None) => {
                // Expect (msg=dict, pk) as literal args
                let msg_v = match op.1.first() {
                    Some(pod2::frontend::OperationArg::Literal(v)) => v,
                    _ => {
                        return Err(Error::Msg(
                            "SignedBy op missing or non-literal message argument".into(),
                        ))
                    }
                };
                let pk_v = match op.1.get(1) {
                    Some(pod2::frontend::OperationArg::Literal(v)) => v,
                    _ => {
                        return Err(Error::Msg(
                            "SignedBy op missing or non-literal public key argument".into(),
                        ))
                    }
                };
                let ok_msg = match msg_v.typed() {
                    TypedValue::Dictionary(d) => d.commitment() == signed.dict.commitment(),
                    _ => false,
                };
                let ok_pk = match pk_v.typed() {
                    TypedValue::PublicKey(pk) => pk == &signed.public_key,
                    _ => false,
                };
                if !(ok_msg && ok_pk) {
                    return Err(Error::Msg(
                        "SignedBy op args do not match provided signature".into(),
                    ));
                }
                // Build pk as a Contains statement anchored to the signed action body.
                // NOTE: We assume the reducer used `a.get("user")` in its SignedBy guard.
                // If a different key is used, this will need to be parameterized.
                let pk_stmt = signed
                    .get_statement("user")
                    .ok_or_else(|| Error::Msg("SignedDict missing 'user' field".into()))?;
                out.push(FOperation::signed_by(
                    Value::from(signed.dict.clone()),
                    pk_stmt,
                    signed.signature.clone(),
                ));
            }
            _ => out.push(op),
        }
    }
    Ok(out)
}
