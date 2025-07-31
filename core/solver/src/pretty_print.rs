//! Pretty-printing utilities for debug logs and trace output.
//!
//! This module provides human-readable formatting for internal solver data structures
//! that are frequently logged during debugging. The goal is to preserve all essential
//! debugging information while making logs readable and concise.
//!
//! ## Usage Guidelines
//!
//! ### In Log Statements
//! Use the wrapper structs for Display trait implementations:
//! ```text
//! use crate::pretty_print::*;
//!
//! // Good - using pretty-print wrapper
//! log::debug!("Rule: {}", PrettyRule(rule));
//! log::debug!("Bindings: {}", PrettyBindings(&bindings));
//!
//! // Avoid - using raw Debug formatting
//! log::debug!("Rule: {:?}", rule);
//! ```
//!
//! ### Format Functions
//! For inline formatting within larger strings, use the format functions:
//! ```text
//! log::debug!("Processing {} with {} facts",
//!            format_predicate_identifier(&pred_id),
//!            facts.len());
//! ```
//!
//! ### Consistency
//! - Always use pretty-printing for complex data structures in logs
//! - Use consistent formatting across all log levels (debug, info, trace, etc.)
//! - Prefer wrapper structs over format functions when possible
//! - Use meaningful prefixes in log messages to provide context

use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

use pod2::{
    lang::PrettyPrint,
    middleware::{
        CustomPredicateRef, Hash, Predicate, StatementTmpl, StatementTmplArg, Value, ValueRef,
        Wildcard,
    },
};

use crate::{
    engine::semi_naive::{Fact, FactStore},
    ir::{Atom, PredicateIdentifier, Rule},
};

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
        StatementTmplArg::Literal(value) => value.to_podlang_string(),
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
        PredicateIdentifier::Normal(Predicate::Native(native)) => format!("{native:?}"),
        PredicateIdentifier::Normal(Predicate::Custom(cpr)) => format_custom_predicate_ref(cpr),
        PredicateIdentifier::Normal(Predicate::BatchSelf(idx)) => format!("BatchSelf({idx})"),
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
        format!("{head}.")
    } else {
        let body_atoms: Vec<String> = rule.body.iter().map(format_atom).collect();
        format!("{} :- {}.", head, body_atoms.join(", "))
    }
}

/// Pretty-print a StatementTmpl
pub fn format_statement_template(stmt: &StatementTmpl) -> String {
    let pred_name = match &stmt.pred {
        Predicate::Native(native) => format!("{native:?}"),
        Predicate::Custom(cpr) => format_custom_predicate_ref(cpr),
        Predicate::BatchSelf(idx) => format!("BatchSelf({idx})"),
    };
    let args: Vec<String> = stmt.args.iter().map(format_statement_arg).collect();
    format!("{}({})", pred_name, args.join(", "))
}

/// Pretty-print a HashMap of variable bindings
pub fn format_bindings(bindings: &HashMap<Wildcard, Value>) -> String {
    let mut items: Vec<String> = bindings
        .iter()
        .map(|(wildcard, value)| format!("{}: {}", format_wildcard(wildcard), value))
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
                items.push(format!("{pred_name}: {fact_count} facts"));
            }
        }
    }
    format!("{{{}}}", items.join(", "))
}

/// Pretty-print a ValueRef
pub fn format_value_ref(value_ref: &ValueRef) -> String {
    match value_ref {
        ValueRef::Literal(value) => value.to_podlang_string(),
        ValueRef::Key(ak) => format!("{}[{}]", ak.pod_id, ak.key.name()),
    }
}

/// Pretty-print a Vec of Values for materializer logs
pub fn format_value_vec(values: &[Option<Value>]) -> String {
    let formatted: Vec<String> = values
        .iter()
        .map(|opt_val| match opt_val {
            Some(val) => val.to_podlang_string(),
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

/// Wrapper struct for pretty-printing a single Value
pub struct PrettyValue<'a>(pub &'a Value);

impl Display for PrettyValue<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

/// Wrapper struct for pretty-printing a single Wildcard
pub struct PrettyWildcard<'a>(pub &'a Wildcard);

impl Display for PrettyWildcard<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_wildcard(self.0))
    }
}

/// Wrapper struct for pretty-printing a StatementTmplArg
pub struct PrettyStatementTmplArg<'a>(pub &'a StatementTmplArg);

impl Display for PrettyStatementTmplArg<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_statement_arg(self.0))
    }
}

/// Wrapper struct for pretty-printing a PredicateIdentifier
pub struct PrettyPredicateIdentifier<'a>(pub &'a PredicateIdentifier);

impl Display for PrettyPredicateIdentifier<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", format_predicate_identifier(self.0))
    }
}

/// Wrapper struct for pretty-printing iteration summary information
pub struct PrettyIterationSummary {
    pub iteration: usize,
    pub new_facts: usize,
    pub total_facts: usize,
}

impl Display for PrettyIterationSummary {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "Iteration {} complete. New facts: {}, Total facts: {}",
            self.iteration, self.new_facts, self.total_facts
        )
    }
}

/// Wrapper struct for pretty-printing rule evaluation context
pub struct PrettyRuleEvaluation<'a> {
    pub rule: &'a Rule,
    pub bindings_count: usize,
    pub new_facts_count: usize,
}

impl Display for PrettyRuleEvaluation<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "Rule {} produced {} new facts from {} bindings",
            format_rule(self.rule),
            self.new_facts_count,
            self.bindings_count
        )
    }
}

/// Wrapper struct for pretty-printing materialization results
pub struct PrettyMaterializationResult<'a> {
    pub predicate: &'a PredicateIdentifier,
    pub bindings: &'a HashMap<Wildcard, Value>,
    pub result_count: usize,
}

impl Display for PrettyMaterializationResult<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "Materialized {} with {} -> {} facts",
            format_predicate_identifier(self.predicate),
            format_bindings(self.bindings),
            self.result_count
        )
    }
}

/// Wrapper struct for pretty-printing database queries
pub struct PrettyDatabaseQuery<'a> {
    pub batch_id: &'a pod2::middleware::Hash,
    pub pred_idx: usize,
    pub binding_vector: &'a [Option<Value>],
}

impl Display for PrettyDatabaseQuery<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "Query {}[{}] with bindings [{}]",
            format_hash(self.batch_id),
            self.pred_idx,
            self.binding_vector
                .iter()
                .map(|opt_val| match opt_val {
                    Some(val) => val.to_podlang_string(),
                    None => "?".to_string(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Wrapper struct for pretty-printing join failure details
pub struct PrettyJoinFailure<'a> {
    pub literal: &'a Atom,
    pub reason: &'a str,
}

impl Display for PrettyJoinFailure<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "Join failed on {} ({})",
            format_atom(self.literal),
            self.reason
        )
    }
}

#[cfg(test)]
mod tests {
    use pod2::middleware::Value;

    use super::*;

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

    #[test]
    fn test_pretty_value_wrapper() {
        let value = Value::from(42i64);
        let pretty_value = PrettyValue(&value);
        assert_eq!(pretty_value.to_string(), "42");

        let string_value = Value::from("hello");
        let pretty_string = PrettyValue(&string_value);
        assert_eq!(pretty_string.to_string(), "\"hello\"");
    }

    #[test]
    fn test_pretty_wildcard_wrapper() {
        let wildcard = Wildcard::new("test_var".to_string(), 0);
        let pretty_wildcard = PrettyWildcard(&wildcard);
        assert_eq!(pretty_wildcard.to_string(), "?test_var");
    }

    #[test]
    fn test_pretty_iteration_summary() {
        let summary = PrettyIterationSummary {
            iteration: 5,
            new_facts: 10,
            total_facts: 42,
        };
        assert_eq!(
            summary.to_string(),
            "Iteration 5 complete. New facts: 10, Total facts: 42"
        );
    }
}
