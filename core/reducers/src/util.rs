use pod2::middleware::{
    containers::{Array as MwArray, Dictionary as MwDict, Set as MwSet},
    TypedValue, Value,
};

use crate::error::{Error, Result};

pub fn as_dict(v: &Value) -> Result<MwDict> {
    match v.typed() {
        TypedValue::Dictionary(d) => Ok(d.clone()),
        other => Err(Error::Type(format!("Expected Dictionary, got {other}"))),
    }
}
pub fn as_set(v: &Value) -> Result<MwSet> {
    match v.typed() {
        TypedValue::Set(s) => Ok(s.clone()),
        other => Err(Error::Type(format!("Expected Set, got {other}"))),
    }
}
pub fn as_array(v: &Value) -> Result<MwArray> {
    match v.typed() {
        TypedValue::Array(a) => Ok(a.clone()),
        other => Err(Error::Type(format!("Expected Array, got {other}"))),
    }
}

pub fn int_from_value(v: &Value) -> Result<i64> {
    if let TypedValue::Int(i) = v.typed() {
        Ok(*i)
    } else {
        Err(Error::Type(format!("Expected Int, got {}", v.typed())))
    }
}
