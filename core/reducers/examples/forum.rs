use pod2::{
    backends::plonky2::{mock::mainpod::MockProver, signer::Signer},
    examples::MOCK_VD_SET,
    frontend::SignedDict,
    lang::PrettyPrint,
    middleware::{
        containers::{Dictionary, Set},
        Key, Params, SecretKey, TypedValue, Value,
    },
};
use pod2_reducers::{reducer, reducer_router};

reducer!(forum_create_post, "Forum.CreatePost", |ctx| {
    let state = ctx.state();
    let action = ctx.action();

    // Guards
    ctx.require_signed_by(&action, action.get("user"));
    ctx.require_set_contains(state.get("users"), action.get("user"));
    ctx.require_dict_not_contains(state.get("posts"), action.get("post_id"));

    // Build post' from a fresh empty dict
    let post = ctx
        .empty_dict()
        .set("title", action.get("title"))
        .set("body", action.get("body"))
        .set("upvotes", 0)
        .set("author", action.get("user"))
        .set("voters", ctx.empty_set());

    // posts' = Insert(posts, post_id, post')
    let posts1 = state.get("posts").put(action.get("post_id"), post);

    // Next
    state.set("posts", posts1)
});

reducer!(forum_upvote_post, "Forum.UpvotePost", |ctx| {
    let s = ctx.state();
    let a = ctx.action();
    let pid = a.get("post_id");

    // Guards
    ctx.require_signed_by(&a, a.get("user"));
    ctx.require_set_contains(s.get("users"), a.get("user"));

    // Dynamic lookup of post to read nested fields
    let posts = s.get("posts");
    let post = posts.get(&pid);
    let post1 = post
        .set("upvotes", post.get("upvotes") + 1)
        .set("voters", post.get("voters").insert(a.get("user")));

    // posts' = Update(posts, pid, post')
    let posts1 = posts.set(&pid, post1);

    s.set("posts", posts1)
});

reducer!(forum_delete_post, "Forum.DeletePost", |ctx| {
    let s = ctx.state();
    let a = ctx.action();

    // Guards
    ctx.require_signed_by(&a, a.get("user"));
    let post = s.get("posts").get(a.get("post_id"));
    ctx.require_eq(post.get("author"), a.get("user"));

    let posts1 = s.get("posts").remove(a.get("post_id"));
    s.set("posts", posts1)
});

reducer_router!(forum_router, [
    "Forum.CreatePost"  => forum_create_post,
    "Forum.UpvotePost"  => forum_upvote_post,
    "Forum.DeletePost"  => forum_delete_post,
]);

fn dict_get_i64(d: &Dictionary, k: &str) -> i64 {
    match d.get(&Key::from(k)).unwrap().typed() {
        TypedValue::Int(i) => *i,
        _ => panic!("{k} not int"),
    }
}

fn pp_dict(d: &Dictionary) -> String {
    Value::from(d.clone()).to_podlang_string()
}

fn main() -> pod2_reducers::Result<()> {
    // Build initial forum state
    let params = Params {
        max_custom_predicate_arity: 64,
        max_custom_predicate_wildcards: 64,
        max_merkle_tree_state_transition_proofs_containers: 10,
        max_operation_args: 64,
        ..Default::default()
    };

    let alice_secret_key = SecretKey::new_rand();
    let alice_public_key = alice_secret_key.public_key();
    let signer = Signer(alice_secret_key);

    // Pretty-print Podlang for each reducer
    println!(
        "--- Podlang: Forum.CreatePost ---\n{}",
        forum_create_post::predicate(&params).to_podlang_string()
    );
    println!(
        "--- Podlang: Forum.UpvotePost ---\n{}",
        forum_upvote_post::predicate(&params).to_podlang_string()
    );
    println!(
        "--- Podlang: Forum.DeletePost ---\n{}",
        forum_delete_post::predicate(&params).to_podlang_string()
    );
    println!(
        "--- Podlang: Forum Router (OR) ---\n{}",
        forum_router::predicate(&params).to_podlang_string()
    );
    let mut users = Set::new(params.max_depth_mt_containers, Default::default()).unwrap();
    let _ = users.insert(&Value::from(alice_public_key));

    let posts = pod2::dict!(params.max_depth_mt_containers, {})?;
    let mut state: Dictionary = pod2::dict!(
        params.max_depth_mt_containers,
        {"users" => users, "posts" => posts}
    )?;
    println!("Initial forum state:\n{}\n", pp_dict(&state));

    // CreatePost
    let action_create: SignedDict = pod2_reducers::signed_action!(
        params,
        signer,
        "Forum.CreatePost",
        {"post_id" => "p1", "user" => alice_public_key, "title" => "Hello", "body" => "World"}
    );
    println!("Action (CreatePost):\n{}\n", pp_dict(&action_create.dict));
    let prev0 = state.clone();
    state = forum_router::exec_signed(&prev0, &action_create)?;
    // Build and print POD with only router-level predicate public
    let prover = MockProver {};
    let pod0 = forum_router::build_pod_signed(
        &params,
        &MOCK_VD_SET,
        &prev0,
        &action_create,
        &state,
        &prover,
    )?;
    println!("MainPod after CreatePost:\n{pod0}\n");
    println!("After CreatePost:\n{}\n", pp_dict(&state));
    {
        let posts = match state.get(&"posts".into())?.typed() {
            TypedValue::Dictionary(d) => d,
            _ => unreachable!(),
        };
        assert!(posts.get(&"p1".into()).is_ok());
    }

    // UpvotePost
    let action_upvote: SignedDict = pod2_reducers::signed_action!(params, signer, "Forum.UpvotePost", {"post_id" => "p1", "user" => alice_public_key});
    println!("Action (UpvotePost):\n{}\n", pp_dict(&action_upvote.dict));
    let prev1 = state.clone();
    state = forum_router::exec_signed(&prev1, &action_upvote)?;
    let pod1 = forum_router::build_pod_signed(
        &params,
        &MOCK_VD_SET,
        &prev1,
        &action_upvote,
        &state,
        &prover,
    )?;
    println!("MainPod after UpvotePost:\n{pod1}\n");
    println!("After UpvotePost:\n{}\n", pp_dict(&state));
    {
        let posts = match state.get(&"posts".into())?.typed() {
            TypedValue::Dictionary(d) => d,
            _ => unreachable!(),
        };
        let p1 = posts.get(&"p1".into())?.clone();
        let p1d = match p1.typed() {
            TypedValue::Dictionary(d) => d,
            _ => unreachable!(),
        };
        assert_eq!(dict_get_i64(p1d, "upvotes"), 1);
    }

    // DeletePost
    let action_delete: SignedDict = pod2_reducers::signed_action!(params, signer, "Forum.DeletePost", {"post_id" => "p1", "user" => alice_public_key});
    println!("Action (DeletePost):\n{}\n", pp_dict(&action_delete.dict));
    let prev2 = state.clone();
    state = forum_router::exec_signed(&prev2, &action_delete)?;
    let pod2 = forum_router::build_pod_signed(
        &params,
        &MOCK_VD_SET,
        &prev2,
        &action_delete,
        &state,
        &prover,
    )?;
    println!("MainPod after DeletePost:\n{pod2}\n");
    println!("After DeletePost:\n{}\n", pp_dict(&state));
    {
        let posts = match state.get(&"posts".into())?.typed() {
            TypedValue::Dictionary(d) => d,
            _ => unreachable!(),
        };
        assert!(posts.get(&"p1".into()).is_err());
    }

    println!("Example ran successfully");
    Ok(())
}
