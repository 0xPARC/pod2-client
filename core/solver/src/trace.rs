//! Tracing and debugging utilities for the solver.
//!
//! This module provides structured tracing capabilities for understanding
//! solver behavior, particularly for debugging issues like infinite loops
//! in recursive predicates.

use std::{collections::HashSet, time::Instant};

use hex::ToHex;
use pod2::middleware::CustomPredicateRef;

/// Extension trait for generating unique identifiers for predicates
pub trait PredicateIdentifier {
    /// Generate a debug-friendly identifier: `{batch_id_prefix}::{predicate_name}`
    fn debug_identifier(&self) -> String;

    /// Generate a unique identifier: `{batch_id_prefix}::{predicate_name}[{index}]`
    fn unique_identifier(&self) -> String;
}

impl PredicateIdentifier for CustomPredicateRef {
    fn debug_identifier(&self) -> String {
        let batch_id_hex = self.batch.id().encode_hex::<String>();
        let batch_prefix = &batch_id_hex[..8.min(batch_id_hex.len())];
        format!("{}::{}", batch_prefix, self.predicate().name)
    }

    fn unique_identifier(&self) -> String {
        let batch_id_hex = self.batch.id().encode_hex::<String>();
        let batch_prefix = &batch_id_hex[..8.min(batch_id_hex.len())];
        format!(
            "{}::{}[{}]",
            batch_prefix,
            self.predicate().name,
            self.index
        )
    }
}

/// Configuration for selective tracing
#[derive(Debug, Clone)]
pub struct TraceConfig {
    /// Patterns to match for tracing predicates
    /// Examples:
    /// - "upvote_count" - matches any upvote_count predicate
    /// - "4e5b77a2::upvote_count" - matches specific batch
    /// - "4e5b77a2::*" - matches all predicates in batch
    /// - "*::upvote_count" - matches upvote_count in any batch
    pub trace_patterns: Vec<String>,

    /// Enable tracing of Magic Set transformation
    pub trace_magic_set: bool,

    /// Enable tracing of constraint propagation
    pub trace_constraints: bool,

    /// Maximum number of events to collect
    pub max_events: usize,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            trace_patterns: vec!["*".to_string()], // Trace everything by default
            trace_magic_set: true,
            trace_constraints: true,
            max_events: 1000,
        }
    }
}

impl TraceConfig {
    /// Create a new trace config for specific predicates
    pub fn for_predicates(patterns: Vec<&str>) -> Self {
        Self {
            trace_patterns: patterns.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    /// Check if a predicate should be traced
    pub fn should_trace_predicate(&self, predicate_ref: &CustomPredicateRef) -> bool {
        let unique_id = predicate_ref.unique_identifier();
        let debug_id = predicate_ref.debug_identifier();
        let simple_name = &predicate_ref.predicate().name;

        self.trace_patterns.iter().any(|pattern| {
            self.matches_pattern(pattern, &unique_id)
                || self.matches_pattern(pattern, &debug_id)
                || self.matches_pattern(pattern, simple_name)
        })
    }

    /// Check if a pattern matches an identifier
    fn matches_pattern(&self, pattern: &str, identifier: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains("::") {
            // Qualified pattern matching
            self.matches_qualified_pattern(pattern, identifier)
        } else {
            // Simple name matching
            identifier.contains(pattern) || identifier.ends_with(&format!("::{pattern}"))
        }
    }

    /// Match qualified patterns like "4e5b77a2::upvote_count" or "4e5b77a2::*"
    fn matches_qualified_pattern(&self, pattern: &str, identifier: &str) -> bool {
        let parts: Vec<&str> = pattern.split("::").collect();
        if parts.len() != 2 {
            return false;
        }

        let (batch_pattern, name_pattern) = (parts[0], parts[1]);

        // Check if identifier has the right format
        if !identifier.contains("::") {
            return false;
        }

        let id_parts: Vec<&str> = identifier.split("::").collect();
        if id_parts.len() != 2 {
            return false;
        }

        let (id_batch, id_name) = (id_parts[0], id_parts[1]);

        // Match batch prefix
        let batch_matches = batch_pattern == "*" || id_batch.starts_with(batch_pattern);

        // Match name (handle [index] suffix)
        let name_matches = if name_pattern == "*" {
            true
        } else {
            id_name.starts_with(name_pattern)
        };

        batch_matches && name_matches
    }
}

/// Types of trace events
#[derive(Debug, Clone)]
pub enum TraceEventType {
    /// A magic rule was generated during Magic Set transformation
    MagicRuleGenerated {
        bound_indices: Vec<usize>,
        rule_body_size: usize,
    },
    /// Constraint propagation occurred
    ConstraintPropagated {
        bound_vars: Vec<String>,
        newly_bound: Vec<String>,
    },
    /// Recursion was detected
    RecursionDetected {
        depth: usize,
        previous_calls: Vec<String>,
    },
    /// Infinite loop is suspected
    InfiniteLoopSuspected {
        iteration: usize,
        repeating_pattern: String,
    },
}

/// Context information for a trace event
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// Current iteration number
    pub iteration: usize,
    /// Rule index within the current processing
    pub rule_index: usize,
}

/// A single trace event
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// When the event occurred
    pub timestamp: Instant,
    /// The type of event
    pub event_type: TraceEventType,
    /// Unique identifier of the predicate involved
    pub predicate_id: String,
    /// Context information
    pub context: TraceContext,
}

/// Collection of trace events
#[derive(Debug, Clone)]
pub struct TraceCollection {
    /// Configuration used for this trace
    pub config: TraceConfig,
    /// Collected events
    pub events: Vec<TraceEvent>,
    /// Whether the collection was truncated due to max_events limit
    pub truncated: bool,
}

impl TraceCollection {
    /// Create a new trace collection
    pub fn new(config: TraceConfig) -> Self {
        Self {
            config,
            events: Vec::new(),
            truncated: false,
        }
    }

    /// Add a trace event
    pub fn add_event(&mut self, event: TraceEvent) {
        if self.events.len() >= self.config.max_events {
            self.truncated = true;
            return;
        }

        self.events.push(event);
    }

    /// Filter events by predicate pattern
    pub fn filter_events(&self, pattern: &str) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|event| self.config.matches_pattern(pattern, &event.predicate_id))
            .collect()
    }

    /// Get all unique predicate IDs in the trace
    pub fn get_predicate_ids(&self) -> HashSet<String> {
        self.events
            .iter()
            .map(|event| event.predicate_id.clone())
            .collect()
    }

    /// Analyze recursion patterns
    pub fn analyze_recursion(&self) -> RecursionAnalysis {
        let mut recursion_chains = Vec::new();
        let mut current_chain = Vec::new();

        for event in &self.events {
            match &event.event_type {
                TraceEventType::MagicRuleGenerated { .. } => {
                    current_chain.push(event.predicate_id.clone());
                }
                TraceEventType::RecursionDetected { depth, .. } => {
                    recursion_chains.push(RecursionChain {
                        predicates: current_chain.clone(),
                        depth: *depth,
                    });
                    current_chain.clear();
                }
                _ => {}
            }
        }

        RecursionAnalysis {
            chains: recursion_chains,
            max_depth: self
                .events
                .iter()
                .filter_map(|event| match &event.event_type {
                    TraceEventType::RecursionDetected { depth, .. } => Some(*depth),
                    _ => None,
                })
                .max()
                .unwrap_or(0),
        }
    }
}

/// Analysis of recursion patterns in the trace
#[derive(Debug, Clone)]
pub struct RecursionAnalysis {
    /// Detected recursion chains
    pub chains: Vec<RecursionChain>,
    /// Maximum recursion depth observed
    pub max_depth: usize,
}

/// A chain of recursive predicate calls
#[derive(Debug, Clone)]
pub struct RecursionChain {
    /// Predicates in the chain
    pub predicates: Vec<String>,
    /// Depth of the recursion
    pub depth: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_config_matching() {
        let config = TraceConfig::for_predicates(vec!["upvote_count", "4e5b77a2::*"]);

        // Test the pattern matching logic
        assert!(config.matches_pattern("upvote_count", "abcd1234::upvote_count[0]"));
        assert!(config.matches_pattern("4e5b77a2::*", "4e5b77a2::any_predicate[1]"));
        assert!(!config.matches_pattern("other", "abcd1234::upvote_count[0]"));
    }

    #[test]
    fn test_qualified_pattern_matching() {
        let config = TraceConfig::for_predicates(vec!["*::upvote_count"]);

        assert!(config.matches_qualified_pattern("*::upvote_count", "abcd1234::upvote_count[0]"));
        assert!(config.matches_qualified_pattern("4e5b77a2::*", "4e5b77a2::any_predicate[1]"));
        assert!(!config.matches_qualified_pattern("other::test", "abcd1234::upvote_count[0]"));
    }
}
