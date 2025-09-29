#[cfg(test)]
use pod2::{
    lang::PrettyPrint,
    middleware::{containers::Set, NativeOperation, OperationType, Params, TypedValue},
};

use crate::{build_predicate, util::as_set, BatchBuilder, LoweredPredicate};

// Test-only reducers and router for dispatch tests
crate::reducer!(r_one, "Tag.One", |ctx| {
    let s = ctx.state();
    let a = ctx.action();
    ctx.require_eq(a.get("id"), 1);
    s.update().set("x", 1).build()
});
crate::reducer!(r_two, "Tag.Two", |ctx| {
    let s = ctx.state();
    let a = ctx.action();
    ctx.require_eq(a.get("id"), 2);
    s.update().set("y", 2).build()
});
crate::reducer_router!(router_mod, [
    "Tag.One" => r_one,
    "Tag.Two" => r_two,
]);

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

// Same flow as above but using higher-level wrappers for Posts and Forum
#[test]
fn forum_posts_wrappers_flow() -> crate::error::Result<()> {
    use pod2::middleware::{
        containers::Dictionary, CustomPredicateBatch, CustomPredicateRef, Value,
    };

    crate::reducer!(post_create2, "Post.Create2", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        let p1 = st.dict_insert("title", ac.get("title"));
        let p2 = p1.dict_insert("body", ac.get("body"));
        let p3 = p2.dict_update("upvotes", 0);
        let p4 = p3.dict_update("voters", st.get("voters"));
        p4
    });
    crate::reducer!(post_upvote2, "Post.Upvote2", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        st.update()
            .set("upvotes", st.get("upvotes") + 1)
            .set("voters", st.get("voters").insert(ac.get("user")))
            .build()
    });
    crate::reducer!(post_delete2, "Post.Delete2", |ctx| {
        let st = ctx.state();
        st.dict_insert("__no_effect__", 0)
    });

    let params = Params::default();
    let pred_c = post_create2::predicate(&params);
    let pred_u = post_upvote2::predicate(&params);
    let pred_d = post_delete2::predicate(&params);
    let batch = CustomPredicateBatch::new(
        &params,
        "PostLeafBatch2".to_string(),
        vec![pred_c.clone(), pred_u.clone(), pred_d.clone()],
    );
    let cpr_c = CustomPredicateRef::new(batch.clone(), 0);
    let cpr_u = CustomPredicateRef::new(batch.clone(), 1);
    let cpr_d = CustomPredicateRef::new(batch.clone(), 2);

    // Forum with empty posts
    let empty_posts: Dictionary = pod2::dict!(params.max_depth_mt_containers, {})?;
    let forum_prev: Dictionary =
        pod2::dict!(params.max_depth_mt_containers, {"posts" => empty_posts.clone()})?;

    // Leaf prev
    let voters_empty =
        pod2::middleware::containers::Set::new(params.max_depth_mt_containers, Default::default())
            .unwrap();
    let prev_post_empty: Dictionary =
        pod2::dict!(params.max_depth_mt_containers, {"voters" => voters_empty, "upvotes" => 0})?;

    // Create
    let a_create: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"post_id" => "p1", "title" => "Hello", "body" => "World"})?;
    let next_post_c = post_create2::exec(&prev_post_empty, &a_create)?;
    let mut b_c = std::collections::HashMap::new();
    b_c.insert("Prev".to_string(), Value::from(prev_post_empty.clone()));
    b_c.insert("Action".to_string(), Value::from(a_create.clone()));
    b_c.insert("Next".to_string(), Value::from(next_post_c.clone()));
    let ops_c = crate::plan_operations(&pred_c, &params, &b_c)?;
    let rec_c = crate::LeafReceipt {
        rule_ref: cpr_c,
        key: "p1".into(),
        effect: crate::ChildEffect::Insert,
        next: next_post_c.clone(),
        ops: ops_c,
    };

    let posts_after_c = crate::posts_apply_change(&empty_posts, rec_c)?;
    let forum_after_c = crate::forum_apply_posts_change(&forum_prev, &posts_after_c)?;

    // Upvote
    let a_up: Dictionary =
        pod2::dict!(params.max_depth_mt_containers, {"post_id" => "p1", "user" => "alice"})?;
    let next_post_u = post_upvote2::exec(&next_post_c, &a_up)?;
    let mut b_u = std::collections::HashMap::new();
    b_u.insert("Prev".to_string(), Value::from(next_post_c.clone()));
    b_u.insert("Action".to_string(), Value::from(a_up.clone()));
    b_u.insert("Next".to_string(), Value::from(next_post_u.clone()));
    let ops_u = crate::plan_operations(&pred_u, &params, &b_u)?;
    let rec_u = crate::LeafReceipt {
        rule_ref: cpr_u,
        key: "p1".into(),
        effect: crate::ChildEffect::Update,
        next: next_post_u.clone(),
        ops: ops_u,
    };

    let posts_after_u = crate::posts_apply_change(&posts_after_c, rec_u)?;
    let forum_after_u = crate::forum_apply_posts_change(&forum_after_c, &posts_after_u)?;

    // Delete
    let a_del: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"post_id" => "p1"})?;
    let _next_post_d = post_delete2::exec(&next_post_u, &a_del)?;
    let mut b_d = std::collections::HashMap::new();
    b_d.insert("Prev".to_string(), Value::from(next_post_u.clone()));
    b_d.insert("Action".to_string(), Value::from(a_del.clone()));
    b_d.insert("Next".to_string(), Value::from(next_post_u.clone()));
    let ops_d = crate::plan_operations(&pred_d, &params, &b_d)?;
    let rec_d = crate::LeafReceipt {
        rule_ref: cpr_d,
        key: "p1".into(),
        effect: crate::ChildEffect::Delete,
        next: next_post_u.clone(),
        ops: ops_d,
    };

    let posts_after_d = crate::posts_apply_change(&posts_after_u, rec_d)?;
    let forum_after_d = crate::forum_apply_posts_change(&forum_after_u, &posts_after_d)?;

    // Assertions
    assert!(posts_after_d.get(&"p1".into()).is_err());
    assert_eq!(
        forum_after_c.get(&"posts".into())?.clone(),
        Value::from(posts_after_c)
    );
    assert_eq!(
        forum_after_u.get(&"posts".into())?.clone(),
        Value::from(posts_after_u)
    );
    assert_eq!(
        forum_after_d.get(&"posts".into())?.clone(),
        Value::from(posts_after_d)
    );
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

#[test]
fn batch_composition_with_custom_call() {
    let params = Params::default();
    let mut bb = BatchBuilder::new(&params);
    bb.add("Child", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        st.update().set("x", ac.get("a") + ac.get("b")).build()
    });
    bb.add("Parent", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        ctx.call("Child", st.get("child"), ac.clone(), st.get("child"));
        st.update().set("p", 1).build()
    });
    let (batch, refs) = bb.build("ReducersBatch");
    assert_eq!(batch.predicates().len(), 2);
    assert!(refs.contains_key("Child"));
    assert!(refs.contains_key("Parent"));
    let parent = &batch.predicates()[1];
    let podlang = parent.to_podlang_string();
    assert!(podlang.contains("Equal("));
}

#[test]
fn router_dispatch_and_batch() -> crate::error::Result<()> {
    let params = Params::default();
    // Exec dispatch for Tag.One
    let prev = pod2::dict!(params.max_depth_mt_containers, {"x" => 0}).unwrap();
    let action_one =
        pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Tag.One", "id" => 1})
            .unwrap();
    let next_one = router_mod::exec(&prev, &action_one)?;
    let x = if let TypedValue::Int(i) = next_one.get(&"x".into()).unwrap().typed() {
        *i
    } else {
        0
    };
    assert_eq!(x, 1);

    // Exec dispatch for Tag.Two
    let prev2 = pod2::dict!(params.max_depth_mt_containers, {"y" => 0}).unwrap();
    let action_two =
        pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Tag.Two", "id" => 2})
            .unwrap();
    let next_two = router_mod::exec(&prev2, &action_two)?;
    let y = if let TypedValue::Int(i) = next_two.get(&"y".into()).unwrap().typed() {
        *i
    } else {
        0
    };
    assert_eq!(y, 2);

    // Batch contains both predicates with tag-index mapping
    let (_batch, refs) = router_mod::batch(&params);
    assert!(refs.contains_key("Tag.One"));
    assert!(refs.contains_key("Tag.Two"));
    Ok(())
}

#[test]
fn router_builder_local() -> crate::error::Result<()> {
    use crate::RouterBuilder;
    let params = Params::default();

    let mut rb = RouterBuilder::new();
    rb.add("Tag.One", r_one::predicate, r_one::exec, r_one::plan)
        .add("Tag.Two", r_two::predicate, r_two::exec, r_two::plan);
    let router = rb.build();

    // Exec via builder router
    let prev = pod2::dict!(params.max_depth_mt_containers, {"x" => 0}).unwrap();
    let action_one =
        pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Tag.One", "id" => 1})
            .unwrap();
    let next_one = router.exec(&prev, &action_one)?;
    let x = if let TypedValue::Int(i) = next_one.get(&"x".into()).unwrap().typed() {
        *i
    } else {
        0
    };
    assert_eq!(x, 1);

    // Batch
    let (_batch, refs) = router.batch(&params);
    assert!(refs.contains_key("Tag.One"));
    assert!(refs.contains_key("Tag.Two"));
    Ok(())
}

#[test]
fn plan_with_custom_call_in_batch() -> crate::error::Result<()> {
    use pod2::{
        frontend::Operation as FOperation,
        middleware::{containers::Dictionary, OperationType, Value},
    };

    use crate::planner::plan_operations_in_batch;

    let params = Params::default();
    let mut bb = BatchBuilder::new(&params);
    bb.add("Child", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        st.update().set("x", ac.get("a") + ac.get("b")).build()
    });
    bb.add("Parent", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        ctx.call("Child", st.get("child"), ac.clone(), st.get("child"));
        st.update().set("p", 1).build()
    });
    let (batch, _refs) = bb.build("ReducersBatch");

    let parent = &batch.predicates()[1];

    // Build bindings for Prev/Action/Next
    let prev: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"p" => 0, "child" => pod2::dict!(params.max_depth_mt_containers, {})?})?;
    let action: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"a" => 1, "b" => 2})?;
    let next: Dictionary = {
        let mut d = prev.clone();
        let _ = d.update(&"p".into(), &Value::from(1))?;
        d
    };

    let mut bindings: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    bindings.insert("Prev".to_string(), Value::from(prev));
    bindings.insert("Action".to_string(), Value::from(action));
    bindings.insert("Next".to_string(), Value::from(next));

    let ops = plan_operations_in_batch(parent, &params, &bindings, &batch)?;
    // Expect at least one custom operation corresponding to the Child call
    assert!(ops
        .iter()
        .any(|op| matches!(op.0, OperationType::Custom(_))));
    Ok(())
}

#[test]
fn plan_with_external_custom_ref() -> crate::error::Result<()> {
    use pod2::middleware::{
        containers::Dictionary, CustomPredicate, NativePredicate, Predicate, StatementTmpl,
        StatementTmplArg, Value, Wildcard,
    };

    let params = Params::default();

    // Build an external custom predicate directly (not via our reducer EDSL)
    // ext(Prev, Action, Next) = AND(Equal(Prev, Prev)) as a trivial predicate
    let names = vec!["Prev".to_string(), "Action".to_string(), "Next".to_string()];
    let stmt = StatementTmpl {
        pred: Predicate::Native(NativePredicate::Equal),
        args: vec![
            StatementTmplArg::Wildcard(Wildcard::new("Prev".to_string(), 0)),
            StatementTmplArg::Wildcard(Wildcard::new("Prev".to_string(), 0)),
        ],
    };
    let ext_pred =
        CustomPredicate::and(&params, "Ext.Trivial".to_string(), vec![stmt], 3, names).unwrap();
    let ext_batch = pod2::middleware::CustomPredicateBatch::new(
        &params,
        "ExtBatch".to_string(),
        vec![ext_pred],
    );
    let ext_ref = pod2::middleware::CustomPredicateRef::new(ext_batch.clone().into(), 0);

    // Build a reducer that references the external custom ref as a guard
    let LoweredPredicate { predicate, .. } = build_predicate(&params, "Parent.UsesExt", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        ctx.require_custom_ref(ext_ref.clone(), vec![st.clone(), ac.clone(), st.clone()]);
        st.update().set("p", 1).build()
    });

    // Plan using the standalone planner (no batch needed for CustomRef)
    let prev: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"p" => 0})?;
    let action: Dictionary = pod2::dict!(params.max_depth_mt_containers, {})?;
    let next: Dictionary = {
        let mut d = prev.clone();
        let _ = d.update(&"p".into(), &Value::from(1))?;
        d
    };
    let mut bindings: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    bindings.insert("Prev".to_string(), Value::from(prev));
    bindings.insert("Action".to_string(), Value::from(action));
    bindings.insert("Next".to_string(), Value::from(next));

    let ops = crate::plan_operations(&predicate, &params, &bindings)?;
    assert!(ops
        .iter()
        .any(|op| matches!(op.0, pod2::middleware::OperationType::Custom(_))));
    Ok(())
}

// Bottom-up execution demo: leaf with private action emits a receipt; parent applies it.
#[test]
fn bottom_up_leaf_receipt_and_parent_apply() -> crate::error::Result<()> {
    use pod2::middleware::{
        containers::Dictionary, CustomPredicateBatch, CustomPredicateRef, Value,
    };

    // Define a simple leaf reducer via macro for determinism and reuse
    crate::reducer!(leaf_inc, "Leaf.Inc", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        // NextLeaf.x = PrevLeaf.x + Action.inc
        st.update().set("x", st.get("x") + ac.get("inc")).build()
    });

    let params = Params {
        max_custom_predicate_arity: 10,
        ..Default::default()
    };

    // Build a batch solely to obtain a concrete CustomPredicateRef for the leaf rule
    let pred = leaf_inc::predicate(&params);
    let batch = CustomPredicateBatch::new(&params, "LeafBatch".to_string(), vec![pred.clone()]);
    let leaf_ref = CustomPredicateRef::new(batch.clone(), 0);

    // Prepare leaf Prev/Action and compute Next via exec (private action)
    let prev_leaf: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"x" => 1})?;
    let action: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"inc" => 2})?;
    let next_leaf = leaf_inc::exec(&prev_leaf, &action)?; // x = 3

    // Materialize leaf operations by planning with bindings (acts as witness computation too)
    let mut bindings: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    bindings.insert("Prev".to_string(), Value::from(prev_leaf.clone()));
    bindings.insert("Action".to_string(), Value::from(action.clone()));
    bindings.insert("Next".to_string(), Value::from(next_leaf.clone()));
    let leaf_ops = crate::plan_operations(&pred, &params, &bindings)?;

    // Package a bottom-up receipt (no action included)
    let receipt = crate::LeafReceipt {
        rule_ref: leaf_ref,
        key: "child".to_string(),
        effect: crate::ChildEffect::Update,
        next: next_leaf.clone(),
        ops: leaf_ops,
    };

    // Parent embeds child.Next into its own dictionary under key "child"
    let prev_parent: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"child" => prev_leaf.clone(), "p" => 0}
    )?;
    let next_parent = crate::apply_child_receipts(&prev_parent, &[receipt])?;

    // Assert parent.child == next_leaf and 'p' unchanged
    let child_after = next_parent.get(&"child".into())?.clone();
    assert_eq!(child_after, Value::from(next_leaf));
    match next_parent.get(&"p".into())?.typed() {
        TypedValue::Int(i) => assert_eq!(*i, 0),
        _ => panic!("p not int"),
    }
    Ok(())
}

// Forum -> Posts example: Create, Upvote, Delete ripple-up
#[test]
fn forum_posts_post_bottom_up_flow() -> crate::error::Result<()> {
    use pod2::middleware::{
        containers::{Dictionary, Set as MwSet},
        CustomPredicateBatch, CustomPredicateRef, Value,
    };

    crate::reducer!(post_create, "Post.Create", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        // Use dict_insert for new keys, dict_update for existing defaults
        let p1 = st.dict_insert("title", ac.get("title"));
        let p2 = p1.dict_insert("body", ac.get("body"));
        let p3 = p2.dict_update("upvotes", 0);
        let p4 = p3.dict_update("voters", st.get("voters"));
        p4
    });
    crate::reducer!(post_upvote, "Post.Upvote", |ctx| {
        let st = ctx.state();
        let ac = ctx.action();
        st.update()
            .set("upvotes", st.get("upvotes") + 1)
            .set("voters", st.get("voters").insert(ac.get("user")))
            .build()
    });
    crate::reducer!(post_delete, "Post.Delete", |ctx| {
        let st = ctx.state();
        // No-op on the leaf; parent will perform DictDelete. Use insert to avoid missing-key update.
        st.dict_insert("__no_effect__", 0)
    });

    let params = Params::default();

    // Build a batch to get refs for leaf rules
    let pred_create = post_create::predicate(&params);
    let pred_upvote = post_upvote::predicate(&params);
    let pred_delete = post_delete::predicate(&params);
    let batch = CustomPredicateBatch::new(
        &params,
        "PostLeafBatch".to_string(),
        vec![
            pred_create.clone(),
            pred_upvote.clone(),
            pred_delete.clone(),
        ],
    );
    let cpr_create = CustomPredicateRef::new(batch.clone(), 0);
    let cpr_upvote = CustomPredicateRef::new(batch.clone(), 1);
    let cpr_delete = CustomPredicateRef::new(batch.clone(), 2);

    // Start with empty forum: { posts: {} }
    let empty_posts: Dictionary = pod2::dict!(params.max_depth_mt_containers, {})?;
    let forum_prev: Dictionary =
        pod2::dict!(params.max_depth_mt_containers, {"posts" => empty_posts.clone()})?;

    // 1) Create a post id "p1"
    // Seed prev post with default containers for smooth creation
    let voters_empty =
        pod2::middleware::containers::Set::new(params.max_depth_mt_containers, Default::default())
            .unwrap();
    let prev_post_empty: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"voters" => voters_empty, "upvotes" => 0}
    )?;
    let action_create: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"post_id" => "p1", "title" => "Hello", "body" => "World"}
    )?;
    let next_post_created = post_create::exec(&prev_post_empty, &action_create)?;
    let mut bindings_create = std::collections::HashMap::new();
    bindings_create.insert("Prev".to_string(), Value::from(prev_post_empty.clone()));
    bindings_create.insert("Action".to_string(), Value::from(action_create.clone()));
    bindings_create.insert("Next".to_string(), Value::from(next_post_created.clone()));
    let ops_create = crate::plan_operations(&pred_create, &params, &bindings_create)?;
    let receipt_create = crate::LeafReceipt {
        rule_ref: cpr_create,
        key: "p1".to_string(),
        effect: crate::ChildEffect::Insert,
        next: next_post_created.clone(),
        ops: ops_create,
    };
    let posts_after_create = crate::apply_child_receipts(&empty_posts, &[receipt_create.clone()])?;
    let forum_after_create: Dictionary = {
        let mut f = forum_prev.clone();
        let _ = f.update(&"posts".into(), &Value::from(posts_after_create.clone()))?;
        f
    };

    // 2) Upvote p1
    let prev_post_for_upvote_dict = next_post_created.clone();
    let action_upvote: Dictionary =
        pod2::dict!(params.max_depth_mt_containers, {"post_id" => "p1", "user" => "alice"})?;
    let next_post_upvoted = post_upvote::exec(&prev_post_for_upvote_dict, &action_upvote)?;
    let mut bindings_upvote = std::collections::HashMap::new();
    bindings_upvote.insert(
        "Prev".to_string(),
        Value::from(prev_post_for_upvote_dict.clone()),
    );
    bindings_upvote.insert("Action".to_string(), Value::from(action_upvote.clone()));
    bindings_upvote.insert("Next".to_string(), Value::from(next_post_upvoted.clone()));
    let ops_upvote = crate::plan_operations(&pred_upvote, &params, &bindings_upvote)?;
    let receipt_upvote = crate::LeafReceipt {
        rule_ref: cpr_upvote,
        key: "p1".to_string(),
        effect: crate::ChildEffect::Update,
        next: next_post_upvoted.clone(),
        ops: ops_upvote,
    };
    let posts_after_upvote = crate::apply_child_receipts(&posts_after_create, &[receipt_upvote])?;
    let forum_after_upvote: Dictionary = {
        let mut f = forum_after_create.clone();
        let _ = f.update(&"posts".into(), &Value::from(posts_after_upvote.clone()))?;
        f
    };

    // 3) Delete p1
    let prev_post_for_delete_dict = next_post_upvoted.clone();
    let action_delete: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"post_id" => "p1", "reason" => "cleanup"}
    )?;
    let _next_post_deleted = post_delete::exec(&prev_post_for_delete_dict, &action_delete)?;
    let mut bindings_delete = std::collections::HashMap::new();
    bindings_delete.insert(
        "Prev".to_string(),
        Value::from(prev_post_for_delete_dict.clone()),
    );
    bindings_delete.insert("Action".to_string(), Value::from(action_delete.clone()));
    // Next is irrelevant for delete at parent level; bind to empty dict for planning
    let empty_post: Dictionary = pod2::dict!(params.max_depth_mt_containers, {})?;
    bindings_delete.insert("Next".to_string(), Value::from(empty_post.clone()));
    let ops_delete = crate::plan_operations(&pred_delete, &params, &bindings_delete)?;
    let receipt_delete = crate::LeafReceipt {
        rule_ref: cpr_delete,
        key: "p1".to_string(),
        effect: crate::ChildEffect::Delete,
        next: empty_post,
        ops: ops_delete,
    };
    let posts_after_delete = crate::apply_child_receipts(&posts_after_upvote, &[receipt_delete])?;
    let forum_after_delete: Dictionary = {
        let mut f = forum_after_upvote.clone();
        let _ = f.update(&"posts".into(), &Value::from(posts_after_delete.clone()))?;
        f
    };

    // Assertions
    // After create: post upvotes=0 (from next_post_created)
    match next_post_created.get(&"upvotes".into())?.typed() {
        TypedValue::Int(i) => assert_eq!(*i, 0),
        _ => panic!("upvotes not int"),
    }
    // After upvote: post upvotes=1 and voters contains alice (from next_post_upvoted)
    match next_post_upvoted.get(&"upvotes".into())?.typed() {
        TypedValue::Int(i) => assert_eq!(*i, 1),
        _ => panic!("upvotes not int"),
    }
    let voters_v = next_post_upvoted.get(&"voters".into())?;
    let s = crate::util::as_set(&voters_v)?;
    assert!(s.set().contains(&Value::from("alice")));
    // After delete: posts dict no longer contains p1
    let has_p1 = posts_after_delete.get(&"p1".into()).is_ok();
    assert!(!has_p1);
    // Forum reflects posts after each step
    assert_eq!(
        forum_after_create.get(&"posts".into())?.clone(),
        Value::from(posts_after_create)
    );
    assert_eq!(
        forum_after_upvote.get(&"posts".into())?.clone(),
        Value::from(posts_after_upvote)
    );
    assert_eq!(
        forum_after_delete.get(&"posts".into())?.clone(),
        Value::from(posts_after_delete)
    );

    Ok(())
}
