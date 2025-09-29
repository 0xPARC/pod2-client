use pod2::{
    frontend::Operation,
    middleware::{containers::Dictionary, CustomPredicateRef, Key, TypedValue, Value},
};

use crate::error::{Error, Result};

/// Effect of a child change as seen by its parent collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChildEffect {
    Insert,
    Update,
    Delete,
}

/// Receipt for a leaf update in bottom-up execution.
/// - `rule_ref`: identifies which predicate proved Prev→Next at the leaf
/// - `key`: the child's key in the parent collection (e.g., post_id)
/// - `effect`: the kind of change for the parent to apply
/// - `next`: the resulting next state for the leaf (ignored for Delete)
/// - `ops`: optional native operations describing the leaf transition (eager materialization)
#[derive(Clone, Debug)]
pub struct LeafReceipt {
    pub rule_ref: CustomPredicateRef,
    pub key: String,
    pub effect: ChildEffect,
    pub next: Dictionary,
    pub ops: Vec<Operation>,
}

/// Apply a set of leaf receipts to a parent dictionary by updating the provided keys.
/// Assumes each receipt corresponds to an immediate child field of the parent.
pub fn apply_child_receipts(
    prev_parent: &Dictionary,
    receipts: &[LeafReceipt],
) -> Result<Dictionary> {
    let mut out = prev_parent.clone();
    for receipt in receipts.iter() {
        let k = Key::from(receipt.key.clone());
        match receipt.effect {
            ChildEffect::Insert => {
                let _ = out
                    .insert(&k, &Value::from(receipt.next.clone()))
                    .map_err(Error::from)?;
            }
            ChildEffect::Update => {
                let _ = out
                    .update(&k, &Value::from(receipt.next.clone()))
                    .map_err(Error::from)?;
            }
            ChildEffect::Delete => {
                let _ = out.delete(&k).map_err(Error::from)?;
            }
        }
    }
    Ok(out)
}

/// Convenience: apply a single child receipt to the `Posts` dictionary.
pub fn posts_apply_change(prev_posts: &Dictionary, receipt: LeafReceipt) -> Result<Dictionary> {
    apply_child_receipts(prev_posts, &[receipt])
}

/// Convenience: update a `Forum` dictionary's `"posts"` field with `next_posts`.
pub fn forum_apply_posts_change(
    prev_forum: &Dictionary,
    next_posts: &Dictionary,
) -> Result<Dictionary> {
    let mut out = prev_forum.clone();
    let _ = out
        .update(&Key::from("posts"), &Value::from(next_posts.clone()))
        .map_err(Error::from)?;
    Ok(out)
}

/// Apply post receipts to the embedded `posts` dictionary in the forum, then write back.
pub fn forum_apply_post_receipts(
    prev_forum: &Dictionary,
    receipts: &[LeafReceipt],
) -> Result<Dictionary> {
    let prev_posts_v = prev_forum
        .get(&Key::from("posts"))
        .map_err(Error::from)?
        .clone();
    let prev_posts = match prev_posts_v.typed() {
        TypedValue::Dictionary(d) => d.clone(),
        other => {
            return Err(Error::Type(format!(
                "Forum.posts not a Dictionary: {}",
                other
            )))
        }
    };
    let next_posts = apply_child_receipts(&prev_posts, receipts)?;
    forum_apply_posts_change(prev_forum, &next_posts)
}
