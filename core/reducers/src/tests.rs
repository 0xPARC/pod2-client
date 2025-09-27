#[cfg(test)]
mod tests {
    use pod2::{
        lang::PrettyPrint,
        middleware::{containers::Set, NativeOperation, OperationType, Params, TypedValue},
    };

    use crate::{build_predicate, util::as_set, LoweredPredicate};

    #[test]
    fn lowers_upvote() {
        let params = Params {
            max_custom_predicate_arity: 10,
            ..Default::default()
        };
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "post_upvote", |ctx| {
            let state = ctx.state();
            let action = ctx.action();
            ctx.require_eq(state.get("post_id"), action.get("post_id"));
            state
                .update()
                .set("upvotes", state.get("upvotes") + 1)
                .set("voters", state.get("voters").insert(action.get("user")))
                .build()
        });
        let podlang = predicate.to_podlang_string();
        assert!(podlang.contains("SumOf"));
        assert!(podlang.contains("DictUpdate"));
        assert!(podlang.contains("SetInsert"));
        assert!(podlang.contains("Equal"));
    }

    #[test]
    fn guard_sum_equal() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Post.Check", |ctx| {
            let state = ctx.state();
            ctx.require_eq(state.get("foo").add(10), state.get("bar"));
            state.update().set("noop", state.get("noop")).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("SumOf"));
        assert!(s.contains("Equal"));
        assert!(s.contains("Prev[\"foo\"]"));
        assert!(s.contains("Prev[\"bar\"]"));
    }

    #[test]
    fn guard_lt() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Lt.Check", |ctx| {
            let st = ctx.state();
            let ac = ctx.action();
            ctx.require_lt(st.get("a"), ac.get("b"));
            st.update().set("x", st.get("x")).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("Lt("));
    }

    #[test]
    fn dict_update_chain_multiple() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Dict.Update", |ctx| {
            let st = ctx.state();
            let ac = ctx.action();
            st.update()
                .set("k1", ac.get("v1"))
                .set("k2", st.get("k2").add(1))
                .build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.matches("DictUpdate(").count() >= 2);
        assert!(s.contains("SumOf"));
    }

    #[test]
    fn set_delete_and_update_parent() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Set.Delete", |ctx| {
            let st = ctx.state();
            let ac = ctx.action();
            let new_set = st.get("voters").delete(ac.get("user"));
            st.update().set("voters", new_set).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("SetDelete("));
        assert!(s.contains("DictUpdate("));
    }

    #[test]
    fn dict_insert_and_delete() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Dict.Mutate", |ctx| {
            let st = ctx.state();
            let ac = ctx.action();
            let meta_ins = st.get("meta").dict_insert("flag", ac.get("flag"));
            let meta_del = meta_ins.dict_delete("old");
            st.update().set("meta", meta_del).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("DictInsert("));
        assert!(s.contains("DictDelete("));
    }

    #[test]
    fn array_update_and_writeback() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Array.Update", |ctx| {
            let st = ctx.state();
            let ac = ctx.action();
            let new_arr = st.get("items").array_update(3, ac.get("value"));
            st.update().set("items", new_arr).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("ArrayUpdate("));
        assert!(s.contains("DictUpdate("));
    }

    #[test]
    fn product_and_max() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Arith.MulMax", |ctx| {
            let st = ctx.state();
            let a = st.get("a");
            let b = st.get("b");
            st.update()
                .set("c", a.clone() * b.clone())
                .set("d", a.max(b))
                .build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("ProductOf("));
        assert!(s.contains("MaxOf("));
    }

    #[test]
    fn operator_add_mul() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Arith.AddMul", |ctx| {
            let st = ctx.state();
            st.update()
                .set("e", st.get("a") + 2)
                .set("f", st.get("b") * 3)
                .build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("SumOf("));
        assert!(s.contains("ProductOf("));
    }

    #[test]
    fn dict_contains_guard() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Dict.Contains", |ctx| {
            let st = ctx.state();
            let ac = ctx.action();
            ctx.require_dict_contains(st.clone(), "k", ac.get("v"));
            st.update().set("x", st.get("x")).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("DictContains("));
    }

    #[test]
    fn signed_by_guard() {
        let params = Params::default();
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "Sig.Check", |ctx| {
            let ac = ctx.action();
            ctx.require_signed_by(ac.get("msg"), ac.get("pubkey"));
            let st = ctx.state();
            st.update().set("ok", 1).build()
        });
        let s = predicate.to_podlang_string();
        assert!(s.contains("SignedBy("));
    }

    #[test]
    fn plan_ops_upvote() -> crate::error::Result<()> {
        let params = Params {
            max_custom_predicate_arity: 10,
            ..Default::default()
        };
        let LoweredPredicate { predicate, .. } = build_predicate(&params, "post_upvote", |ctx| {
            let state = ctx.state();
            let action = ctx.action();
            ctx.require_eq(state.get("post_id"), action.get("post_id"));
            state
                .update()
                .set("upvotes", state.get("upvotes").add(1))
                .set("voters", state.get("voters").insert(action.get("user")))
                .build()
        });

        let voters_set = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
        let prev = pod2::dict!(params.max_depth_mt_containers, {
            "post_id" => 1,
            "upvotes" => 5,
            "voters" => voters_set
        })
        .unwrap();
        let action = pod2::dict!(params.max_depth_mt_containers, {
            "post_id" => 1,
            "user" => "alice"
        })
        .unwrap();

        let upvotes_next = 6i64;
        let mut voters2 = as_set(&prev.clone().get(&"voters".into()).unwrap().clone())?;
        let _ = voters2.insert(&pod2::middleware::Value::from("alice"))?;
        let mut dict1 = prev.clone();
        let _ = dict1.update(
            &"upvotes".into(),
            &pod2::middleware::Value::from(upvotes_next),
        )?;
        let _ = dict1.update(&"voters".into(), &pod2::middleware::Value::from(voters2))?;
        let next = dict1;

        let mut bindings: std::collections::HashMap<String, pod2::middleware::Value> =
            std::collections::HashMap::new();
        bindings.insert("Prev".to_string(), pod2::middleware::Value::from(prev));
        bindings.insert("Action".to_string(), pod2::middleware::Value::from(action));
        bindings.insert("Next".to_string(), pod2::middleware::Value::from(next));

        let ops = crate::plan_operations(&predicate, &params, &bindings)?;
        assert_eq!(ops.len(), 6);

        match &ops[0].0 {
            OperationType::Native(NativeOperation::EqualFromEntries) => {}
            other => panic!("unexpected first op: {other:?}"),
        }
        match &ops[5].0 {
            OperationType::Native(NativeOperation::EqualFromEntries) => {}
            other => panic!("unexpected last op: {other:?}"),
        }

        let dbg_ops = ops
            .iter()
            .map(|op| format!("{op}"))
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(dbg_ops.contains("SumOf"));
        assert!(dbg_ops.contains("SetInsertFromEntries"));
        assert!(dbg_ops.contains("DictUpdateFromEntries"));
        Ok(())
    }

    #[test]
    fn macro_exec_and_plan() -> crate::error::Result<()> {
        crate::reducer!(macro_post_upvote, "Post.Upvote", |ctx| {
            let s = ctx.state();
            let a = ctx.action();
            ctx.require_eq(s.get("post_id"), a.get("post_id"));
            s.update()
                .set("upvotes", s.get("upvotes") + 1)
                .set("voters", s.get("voters").insert(a.get("user")))
                .build()
        });

        let params = Params {
            max_custom_predicate_arity: 10,
            ..Default::default()
        };
        let voters_set = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
        let prev = pod2::dict!(params.max_depth_mt_containers, {"post_id" => 1, "upvotes" => 2, "voters" => voters_set}).unwrap();
        let action =
            pod2::dict!(params.max_depth_mt_containers, {"post_id" => 1, "user" => "bob"}).unwrap();
        let next = macro_post_upvote::exec(&prev, &action)?;
        let upv = if let TypedValue::Int(i) = next.get(&"upvotes".into()).unwrap().typed() {
            *i
        } else {
            0
        };
        assert_eq!(upv, 3);

        let ops = macro_post_upvote::plan(&params, &prev, &action, &next)?;
        let dbg_ops = ops
            .iter()
            .map(|op| format!("{op}"))
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(dbg_ops.contains("SumOf"));
        assert!(dbg_ops.contains("DictUpdateFromEntries"));
        assert!(dbg_ops.contains("SetInsertFromEntries"));
        Ok(())
    }
}
