use pod2::{
    backends::plonky2::{
        mock::mainpod::MockProver, primitives::ec::schnorr::SecretKey, signer::Signer,
    },
    examples::MOCK_VD_SET,
    frontend::{MainPodBuilder, SignedDictBuilder},
    middleware::{containers::Dictionary, AnchoredKey, Key, Params, Statement, Value, ValueRef},
};
use pod2_new_solver::{
    build_pod_from_answer_top_level_public,
    edb::MockEdbView,
    types::{ConstraintStore, OpTag},
};

#[test]
fn replay_builds_pod_with_equal_ak_ak_and_signedby() {
    // Setup params and vd set
    let params = Params::default();
    let vd = &*MOCK_VD_SET;

    // Build two plain dictionaries with same value under different keys
    let d1 = Dictionary::new(
        params.max_depth_mt_containers,
        [(Key::from("a"), Value::from(7))].into(),
    )
    .unwrap();
    let d2 = Dictionary::new(
        params.max_depth_mt_containers,
        [(Key::from("b"), Value::from(7))].into(),
    )
    .unwrap();
    let r1 = d1.commitment();
    let r2 = d2.commitment();
    let ak1 = AnchoredKey::new(r1, Key::from("a"));
    let ak2 = AnchoredKey::new(r2, Key::from("b"));

    // Contains premises (GeneratedContains tags) used to justify the Equal(AK1, AK2)
    let c1 = Statement::Contains(ValueRef::from(r1), ValueRef::from("a"), ValueRef::from(7));
    let c2 = Statement::Contains(ValueRef::from(r2), ValueRef::from("b"), ValueRef::from(7));
    let equal_head = Statement::Equal(ValueRef::Key(ak1.clone()), ValueRef::Key(ak2.clone()));

    // Also build a SignedDict for SignedBy replay
    let signer = Signer(SecretKey::new_rand());
    let mut sdb = SignedDictBuilder::new(&params);
    sdb.insert("att", 1);
    let sd = sdb.sign(&signer).unwrap();
    let sroot = sd.dict.commitment();
    let signed_by_head = Statement::SignedBy(ValueRef::from(sroot), ValueRef::from(sd.public_key));

    // Construct an answer store with both heads and appropriate tags/premises
    let mut store = ConstraintStore::default();
    store.premises.push((
        equal_head.clone(),
        OpTag::Derived {
            premises: vec![
                (
                    c1.clone(),
                    OpTag::GeneratedContains {
                        root: r1,
                        key: Key::from("a"),
                        value: Value::from(7),
                    },
                ),
                (
                    c2.clone(),
                    OpTag::GeneratedContains {
                        root: r2,
                        key: Key::from("b"),
                        value: Value::from(7),
                    },
                ),
            ],
        },
    ));
    store
        .premises
        .push((signed_by_head.clone(), OpTag::FromLiterals));

    // Prepare EDB (evidence)
    let mut edb = MockEdbView::default();
    edb.add_full_dict(d1.clone());
    edb.add_full_dict(d2.clone());
    edb.add_signed_dict(sd.clone());

    // Build the Pod by replaying the answer
    let pod = build_pod_from_answer_top_level_public(
        &store,
        &params,
        vd,
        |b: &MainPodBuilder| b.prove(&MockProver {}).map_err(|e| format!("{e}")),
        &edb,
    )
    .expect("replay failed");

    println!("pod: {pod}");

    // Assert the resulting pod contains Equal(AK,AK) and SignedBy statements
    let pub_sts = &pod.public_statements;
    assert!(pub_sts
        .iter()
        .any(|s| matches!(s, Statement::Equal(ValueRef::Key(a1), ValueRef::Key(a2)) if a1.root == ak1.root && a1.key.hash() == ak1.key.hash() && a2.root == ak2.root && a2.key.hash() == ak2.key.hash())));
    assert!(pub_sts
        .iter()
        .any(|s| matches!(s, Statement::SignedBy(ValueRef::Literal(m), ValueRef::Literal(pk)) if m.raw() == Value::from(sroot).raw() && pk.raw() == Value::from(sd.public_key).raw())));
}
