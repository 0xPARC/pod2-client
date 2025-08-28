use pod2::middleware::{
    Hash, Key, NativePredicate, Params, Predicate, StatementTmpl, StatementTmplArg, Value,
};

pub fn params() -> Params {
    Params::default()
}

pub fn root(name: &str) -> Hash {
    Hash::from(Value::from(name).raw())
}

pub fn key(name: &str) -> Key {
    Key::from(name)
}

pub fn parse_first_tmpl(podlang: &str) -> StatementTmpl {
    use pod2::lang::parse;
    let req = parse(podlang, &Params::default(), &[])
        .expect("parse ok")
        .request;
    req.request_templates.first().cloned().expect("one tmpl")
}

pub fn parse_native_goals(podlang: &str) -> Vec<(NativePredicate, Vec<StatementTmplArg>)> {
    use pod2::lang::parse;
    let processed = parse(podlang, &Params::default(), &[]).expect("parse ok");
    processed
        .request
        .templates()
        .iter()
        .filter_map(|t| match &t.pred {
            Predicate::Native(p) => Some((*p, t.args.clone())),
            _ => None,
        })
        .collect()
}
