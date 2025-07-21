use pod2::{
    backends::plonky2::primitives::ec::curve::Point as PublicKey,
    middleware::{
        Hash, PodId, RawValue, TypedValue, Value,
        containers::{Array, Dictionary, Set},
    },
};

pub mod prover_setup;

/// Utility trait for extracting typed values from pod2::middleware::Value
pub trait ValueExt {
    fn as_i64(&self) -> Option<i64>;
    fn as_str(&self) -> Option<&str>;
    fn as_bool(&self) -> Option<bool>;
    fn as_public_key(&self) -> Option<&PublicKey>;
    fn as_set(&self) -> Option<&Set>;
    fn as_dictionary(&self) -> Option<&Dictionary>;
    fn as_array(&self) -> Option<&Array>;
    fn as_raw(&self) -> Option<&RawValue>;
    fn as_pod_id(&self) -> Option<&PodId>;
    fn as_hash(&self) -> Option<Hash>;
}

impl ValueExt for Value {
    fn as_i64(&self) -> Option<i64> {
        match self.typed() {
            TypedValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self.typed() {
            TypedValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self.typed() {
            TypedValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_public_key(&self) -> Option<&PublicKey> {
        match self.typed() {
            TypedValue::PublicKey(pk) => Some(pk),
            _ => None,
        }
    }

    fn as_set(&self) -> Option<&Set> {
        match self.typed() {
            TypedValue::Set(set) => Some(set),
            _ => None,
        }
    }

    fn as_dictionary(&self) -> Option<&Dictionary> {
        match self.typed() {
            TypedValue::Dictionary(dict) => Some(dict),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&Array> {
        match self.typed() {
            TypedValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    fn as_raw(&self) -> Option<&RawValue> {
        match self.typed() {
            TypedValue::Raw(raw) => Some(raw),
            _ => None,
        }
    }

    fn as_pod_id(&self) -> Option<&PodId> {
        match self.typed() {
            TypedValue::PodId(id) => Some(id),
            _ => None,
        }
    }

    fn as_hash(&self) -> Option<Hash> {
        match self.typed() {
            TypedValue::Raw(raw) => Some(Hash::from(*raw)),
            _ => None,
        }
    }
}
