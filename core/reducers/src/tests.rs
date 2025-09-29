#[cfg(test)]
use pod2::{
    lang::PrettyPrint,
    middleware::{containers::Set, NativeOperation, OperationType, Params, TypedValue},
};

use crate::{build_predicate, util::as_set, LoweredPredicate};

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
    let params = Params {
        max_custom_predicate_arity: 16,
        ..Default::default()
    };
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

#[test]
fn router_dispatch() -> crate::error::Result<()> {
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

    Ok(())
}

#[test]
fn router_builder_local() -> crate::error::Result<()> {
    use crate::RouterBuilder;
    let params = Params::default();

    let mut rb = RouterBuilder::new();
    rb.add("Tag.One", r_one::exec, r_one::plan)
        .add("Tag.Two", r_two::exec, r_two::plan);
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

    Ok(())
}

// Top-level-only API sketch test: Forum.UpvotePost and Forum.DeletePost
#[test]
fn top_level_forum_actions() -> crate::error::Result<()> {
    use pod2::middleware::{containers::Dictionary, Value};

    // Reducers resembling the API sketch
    crate::reducer!(forum_upvote_post, "Forum.UpvotePost", |ctx| {
        let s = ctx.state();
        let a = ctx.action();
        // Patch
        let posts = s.get("posts");
        let pid = a.get("post_id");
        // Dynamic lookup of the post entry
        let post = posts.get(&pid);
        let post1 = post
            .set("upvotes", post.get("upvotes") + 1)
            .set("voters", post.get("voters").insert(a.get("user")));
        let posts1 = posts.set(&pid, post1);
        s.set("posts", posts1)
    });

    crate::reducer!(forum_delete_post, "Forum.DeletePost", |ctx| {
        let s = ctx.state();
        let a = ctx.action();
        let posts1 = s.get("posts").remove(a.get("post_id"));
        s.set("posts", posts1)
    });

    let params = Params::default();

    // Build Prev forum with users and one post p1
    let users = {
        let s = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
        let mut s2 = s;
        let _ = s2.insert(&pod2::middleware::Value::from("alice"));
        s2
    };
    let voters = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
    let post_p1: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"title" => "Hello", "body" => "World", "upvotes" => 0, "voters" => voters}
    )?;
    let posts: Dictionary = {
        let mut d = pod2::dict!(params.max_depth_mt_containers, {})?;
        let _ = d.insert(&"p1".into(), &Value::from(post_p1.clone()))?;
        d
    };
    let prev_forum: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"users" => users, "posts" => posts}
    )?;

    // Upvote p1 by alice
    let action_up: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Forum.UpvotePost", "post_id" => "p1", "user" => "alice"})?;
    let next_forum = forum_upvote_post::exec(&prev_forum, &action_up)?;
    // Assert upvotes became 1 and voters contain alice
    let p1_after = match next_forum.get(&"posts".into())?.typed() {
        TypedValue::Dictionary(d) => d.get(&"p1".into())?.clone(),
        _ => panic!("posts not dict"),
    };
    let p1_dict = crate::util::as_dict(&p1_after)?;
    match p1_dict.get(&"upvotes".into())?.typed() {
        TypedValue::Int(i) => assert_eq!(*i, 1),
        _ => panic!("upvotes not int"),
    }
    let voters_v = p1_dict.get(&"voters".into())?.clone();
    let voters_set = as_set(&voters_v)?;
    assert!(voters_set.set().contains(&Value::from("alice")));

    // Delete p1
    let action_del: Dictionary = pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Forum.DeletePost", "post_id" => "p1"})?;
    let next_forum2 = forum_delete_post::exec(&next_forum, &action_del)?;
    let posts_after = match next_forum2.get(&"posts".into())?.typed() {
        TypedValue::Dictionary(d) => d,
        _ => panic!("posts not dict"),
    };
    assert!(posts_after.get(&"p1".into()).is_err());

    Ok(())
}

// Define reducers and router at module scope for use in test below
mod forum_router_defs {
    crate::reducer!(forum_upvote_post, "Forum.UpvotePost", |ctx| {
        let s = ctx.state();
        let a = ctx.action();
        let posts = s.get("posts");
        let pid = a.get("post_id");
        let post = posts.get(pid.clone());
        let post1 = post
            .set("upvotes", post.get("upvotes") + 1)
            .set("voters", post.get("voters").insert(a.get("user")));
        let posts1 = posts.set(pid, post1);
        s.set("posts", posts1)
    });

    crate::reducer!(forum_delete_post, "Forum.DeletePost", |ctx| {
        let s = ctx.state();
        let a = ctx.action();
        let posts1 = s.get("posts").remove(a.get("post_id"));
        s.set("posts", posts1)
    });

    crate::reducer_router!(forum_router, [
        "Forum.UpvotePost" => forum_upvote_post,
        "Forum.DeletePost" => forum_delete_post,
    ]);
}

// Router exercising top-level reducers (Upvote, Delete) via action_type dispatch
#[test]
fn top_level_forum_router() -> crate::error::Result<()> {
    use pod2::middleware::{containers::Dictionary, Value};

    // Seed state with one post p1 and user alice
    let params = Params::default();
    let mut users = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
    let _ = users.insert(&Value::from("alice"));
    let voters = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
    let post_p1: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"title" => "Hello", "body" => "World", "upvotes" => 0, "voters" => voters}
    )?;
    let posts: Dictionary = {
        let mut d = pod2::dict!(params.max_depth_mt_containers, {})?;
        let _ = d.insert(&"p1".into(), &Value::from(post_p1.clone()))?;
        d
    };
    let mut forum: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"users" => users, "posts" => posts}
    )?;

    // Upvote p1
    let action_up = pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Forum.UpvotePost", "post_id" => "p1", "user" => "alice"})?;
    forum = forum_router_defs::forum_router::exec(&forum, &action_up)?;
    // ensure upvotes incremented
    let p1_after = match forum.get(&"posts".into())?.typed() {
        TypedValue::Dictionary(d) => d.get(&"p1".into())?.clone(),
        _ => unreachable!(),
    };
    let p1d = crate::util::as_dict(&p1_after)?;
    assert!(matches!(
        p1d.get(&"upvotes".into())?.typed(),
        TypedValue::Int(1)
    ));

    // Delete p1
    let action_del = pod2::dict!(params.max_depth_mt_containers, {"action_type" => "Forum.DeletePost", "post_id" => "p1"})?;
    forum = forum_router_defs::forum_router::exec(&forum, &action_del)?;
    let posts_after = match forum.get(&"posts".into())?.typed() {
        TypedValue::Dictionary(d) => d,
        _ => unreachable!(),
    };
    assert!(posts_after.get(&"p1".into()).is_err());

    Ok(())
}
