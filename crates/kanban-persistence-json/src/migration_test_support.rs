//! Test-only seam exposing the pure V14 -> V15 transform to integration
//! tests outside this crate, which cannot reach the `pub(crate)` version
//! used by the migration chain itself.

use serde_json::Value;

pub fn transform_v14_to_v15(envelope: &mut Value) {
    crate::migration::transform_v14_to_v15_value(envelope).expect("transform must not error");
}

/// Fallible variant, for callers comparing this backend's acceptance of an
/// envelope against another backend's. Whether a backfill REJECTS an input
/// is part of what the two must agree on, so the error cannot be unwrapped
/// away.
pub fn try_transform_v14_to_v15(
    envelope: &mut Value,
) -> kanban_persistence::PersistenceResult<bool> {
    crate::migration::transform_v14_to_v15_value(envelope)
}

/// Fallible variant of the V15 -> V16 card-prefix transform, for the same
/// cross-backend comparison purpose as [`try_transform_v14_to_v15`].
pub fn try_transform_v15_to_v16(
    envelope: &mut Value,
) -> kanban_persistence::PersistenceResult<bool> {
    crate::migration::transform_v15_to_v16_value(envelope)
}

/// Fallible variant of the V17 -> V18 prefix-row repair, for the same
/// cross-backend comparison purpose as [`try_transform_v14_to_v15`].
pub fn try_transform_v17_to_v18(
    envelope: &mut Value,
) -> kanban_persistence::PersistenceResult<bool> {
    crate::migration::transform_v17_to_v18_value(envelope)
}
