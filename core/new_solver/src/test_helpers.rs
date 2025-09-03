use pod2::{
    lang::parse,
    middleware::{Hash, Key, Params, StatementTmplArg, Value},
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

pub fn args_from(query: &str) -> Vec<StatementTmplArg> {
    let req = parse(query, &Params::default(), &[])
        .expect("parse ok")
        .request;
    let tmpl = req.request_templates.first().cloned().expect("one tmpl");
    tmpl.args().to_vec()
}
