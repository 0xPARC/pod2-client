use pod2::middleware::{Hash, Key, Params, StatementTmpl, Value};

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
