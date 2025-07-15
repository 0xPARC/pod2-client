//! Pretty-printing utilities for debug logs and trace output.
//!
//! This module provides human-readable formatting for internal solver data structures
//! that are frequently logged during debugging. The goal is to preserve all essential
//! debugging information while making logs readable and concise.

use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

use pod2::middleware::{
    CustomPredicateRef, Hash, Predicate, StatementTmpl, StatementTmplArg, Value, ValueRef, Wildcard,
};

use crate::{
    engine::semi_naive::{Fact, FactStore},
    ir::{Atom, PredicateIdentifier, Rule},
};

/// Pretty-print a Value, showing only the essential typed information
pub fn format_value(value: &Value) -> String {
    match value.typed() {
        pod2::middleware::TypedValue::Int(i) => i.to_string(),
        pod2::middleware::TypedValue::String(s) => format!("\"{}\"", s),
        pod2::middleware::TypedValue::Bool(b) => b.to_string(),
        pod2::middleware::TypedValue::Array(a) => {
            let items: Vec<String> = a.array().iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        pod2::middleware::TypedValue::Dictionary(d) => {
            let items: Vec<String> = d
                .kvs()
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        pod2::middleware::TypedValue::Set(s) => {
            let items: Vec<String> = s.set().iter().map(format_value).collect();
            format!("#{{{}}}", items.join(", "))
        }
        pod2::middleware::TypedValue::PublicKey(pk) => format!("PublicKey({})", pk),
        pod2::middleware::TypedValue::PodId(id) => format!("PodId({})", format_hash(&id.0)),
        pod2::middleware::TypedValue::Raw(raw) => {
            format!("Raw({})", format_hash(&Hash::from(*raw)))
        }
    }
}

/// Pretty-print a Hash, showing only the first 8 characters
pub fn format_hash(hash: &Hash) -> String {
    let hex = hex::ToHex::encode_hex::<String>(hash);
    format!("{}...", &hex[..8.min(hex.len())])
}

/// Pretty-print a Wildcard as a variable name
pub fn format_wildcard(wildcard: &Wildcard) -> String {
    format!("?{}", wildcard.name)
}

/// Pretty-print a StatementTmplArg
pub fn format_statement_arg(arg: &StatementTmplArg) -> String {
    match arg {
        StatementTmplArg::Literal(value) => format_value(value),
        StatementTmplArg::Wildcard(wildcard) => format_wildcard(wildcard),
        StatementTmplArg::AnchoredKey(wildcard, key) => {
            format!("{}[{}]", format_wildcard(wildcard), key)
        }
        StatementTmplArg::None => "None".to_string(),
    }
}

/// Pretty-print a predicate identifier
pub fn format_predicate_identifier(pred: &PredicateIdentifier) -> String {
    match pred {
        PredicateIdentifier::Normal(Predicate::Native(native)) => format!("{:?}", native),
        PredicateIdentifier::Normal(Predicate::Custom(cpr)) => format_custom_predicate_ref(cpr),
        PredicateIdentifier::Normal(Predicate::BatchSelf(idx)) => format!("BatchSelf({})", idx),
        PredicateIdentifier::Magic {
            name,
            bound_indices,
        } => {
            format!(
                "magic_{}[{}]",
                name,
                bound_indices
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

/// Pretty-print a CustomPredicateRef
pub fn format_custom_predicate_ref(cpr: &CustomPredicateRef) -> String {
    let batch_prefix = format_hash(&cpr.batch.id());
    format!("{}::{}[{}]", batch_prefix, cpr.predicate().name, cpr.index)
}

/// Pretty-print an Atom (predicate call)
pub fn format_atom(atom: &Atom) -> String {
    let pred_name = format_predicate_identifier(&atom.predicate);
    let args: Vec<String> = atom.terms.iter().map(format_statement_arg).collect();
    format!("{}({})", pred_name, args.join(", "))
}

/// Pretty-print a Rule
pub fn format_rule(rule: &Rule) -> String {
    let head = format_atom(&rule.head);
    if rule.body.is_empty() {
        format!("{}.", head)
    } else {
        let body_atoms: Vec<String> = rule.body.iter().map(format_atom).collect();
        format!("{} :- {}.", head, body_atoms.join(", "))
    }
}

/// Pretty-print a StatementTmpl
pub fn format_statement_template(stmt: &StatementTmpl) -> String {
    let pred_name = match &stmt.pred {
        Predicate::Native(native) => format!("{:?}", native),
        Predicate::Custom(cpr) => format_custom_predicate_ref(cpr),
        Predicate::BatchSelf(idx) => format!("BatchSelf({})", idx),
    };
    let args: Vec<String> = stmt.args.iter().map(format_statement_arg).collect();
    format!("{}({})", pred_name, args.join(", "))
}

/// Pretty-print a HashMap of variable bindings
pub fn format_bindings(bindings: &HashMap<Wildcard, Value>) -> String {
    let mut items: Vec<String> = bindings
        .iter()
        .map(|(wildcard, value)| format!("{}: {}", format_wildcard(wildcard), format_value(value)))
        .collect();
    items.sort(); // Consistent ordering
    format!("{{{}}}", items.join(", "))
}

/// Pretty-print a Fact with its arguments
pub fn format_fact(fact: &Fact) -> String {
    let formatted_args: Vec<String> = fact.args.iter().map(format_value_ref).collect();
    format!("({})", formatted_args.join(", "))
}

/// Pretty-print a FactStore for debugging
pub fn format_fact_store(facts: &FactStore) -> String {
    let mut items = Vec::new();
    for (pred_id, fact_set) in facts.iter() {
        let pred_name = format_predicate_identifier(pred_id);
        let fact_count = fact_set.len();
        if fact_count > 0 {
            if fact_count <= 3 {
                // Show individual facts if there are few
                let fact_strings: Vec<String> = fact_set.iter().map(format_fact).collect();
                items.push(format!("{}: [{}]", pred_name, fact_strings.join(", ")));
            } else {
                // Show count if there are many
                items.push(format!("{}: {} facts", pred_name, fact_count));
            }
        }
    }
    format!("{{{}}}", items.join(", "))
}

/// Pretty-print a ValueRef
pub fn format_value_ref(value_ref: &ValueRef) -> String {
    match value_ref {
        ValueRef::Literal(value) => format_value(value),
        ValueRef::Key(ak) => format!("{}[{}]", ak.pod_id, ak.key.name()),
    }
}

/// Pretty-print a Vec of Values for materializer logs
pub fn format_value_vec(values: &[Option<Value>]) -> String {
    let formatted: Vec<String> = values
        .iter()
        .map(|opt_val| match opt_val {
            Some(val) => format_value(val),
            None => "None".to_string(),
        })
        .collect();
    format!("[{}]", formatted.join(", "))
}

/// Pretty-print a Vec of ValueRefs for materializer logs
pub fn format_value_ref_vec(values: &[Option<ValueRef>]) -> String {
    let formatted: Vec<String> = values
        .iter()
        .map(|opt_val| match opt_val {
            Some(val) => format_value_ref(val),
            None => "None".to_string(),
        })
        .collect();
    format!("[{}]", formatted.join(", "))
}

/// Wrapper struct for pretty-printing Rules in logs
pub struct PrettyRule<'a>(pub &'a Rule);

impl Display for PrettyRule<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_rule(self.0))
    }
}

/// Wrapper struct for pretty-printing Atoms in logs
pub struct PrettyAtom<'a>(pub &'a Atom);

impl Display for PrettyAtom<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_atom(self.0))
    }
}

/// Wrapper struct for pretty-printing variable bindings
pub struct PrettyBindings<'a>(pub &'a HashMap<Wildcard, Value>);

impl Display for PrettyBindings<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_bindings(self.0))
    }
}

/// Wrapper struct for pretty-printing FactStore
pub struct PrettyFactStore<'a>(pub &'a FactStore);

impl Display for PrettyFactStore<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_fact_store(self.0))
    }
}

/// Wrapper struct for pretty-printing Value vectors
pub struct PrettyValueVec<'a>(pub &'a [Option<Value>]);

impl Display for PrettyValueVec<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_value_vec(self.0))
    }
}

/// Wrapper struct for pretty-printing ValueRef vectors
pub struct PrettyValueRefVec<'a>(pub &'a [Option<ValueRef>]);

impl Display for PrettyValueRefVec<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_value_ref_vec(self.0))
    }
}

#[cfg(test)]
mod tests {
    use pod2::middleware::Value;

    use super::*;

    #[test]
    fn test_format_value() {
        let int_val = Value::from(42i64);
        assert_eq!(format_value(&int_val), "42");

        let string_val = Value::from("hello");
        assert_eq!(format_value(&string_val), "\"hello\"");

        let bool_val = Value::from(true);
        assert_eq!(format_value(&bool_val), "true");
    }

    #[test]
    fn test_format_wildcard() {
        let wildcard = Wildcard::new("count".to_string(), 0);
        assert_eq!(format_wildcard(&wildcard), "?count");
    }

    #[test]
    fn test_format_bindings() {
        let mut bindings = HashMap::new();
        bindings.insert(Wildcard::new("count".to_string(), 0), Value::from(42i64));
        bindings.insert(Wildcard::new("name".to_string(), 1), Value::from("alice"));

        let formatted = format_bindings(&bindings);
        // Should be sorted by key for consistent output
        assert!(formatted.contains("?count: 42"));
        assert!(formatted.contains("?name: \"alice\""));
    }
}
