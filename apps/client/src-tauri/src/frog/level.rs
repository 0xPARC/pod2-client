use std::sync::Arc;

use pod2::middleware::{CustomPredicateBatch, Params, SELF_ID_HASH};

fn build_level_up_pred() -> Arc<CustomPredicateBatch> {
    let st = format!(
        r#"
        level_up(origin_pod, level, private: proof_pod, shorter_level) = OR(
            level_up_base(?origin_pod, ?level)
            level_up_rec(?origin_pod, ?level, ?proof_pod, ?shorter_level)
        )

        level_up_base(origin_pod, level) = AND(
            Equal(?level, 1)
            Equal(?origin_pod["biome"], 1)
        )

        level_up_rec(origin_pod, level, proof_pod, shorter_level) = AND(
            level_up(?origin_pod, ?shorter_level)
            SumOf(?level, ?shorter_level, 1)
            Equal(?proof_pod["level"], ?level)
            Equal(?proof_pod, Raw({SELF_ID_HASH:#}))
        )
    "#,
    );
    pod2::lang::parse(&st, &Params::default(), &[])
        .unwrap()
        .custom_batch
}
