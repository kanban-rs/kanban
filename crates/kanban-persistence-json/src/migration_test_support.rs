//! Test-only seam exposing the pure V14 -> V15 transform to integration
//! tests outside this crate, which cannot reach the `pub(crate)` version
//! used by the migration chain itself.

use serde_json::Value;

pub fn transform_v14_to_v15(envelope: &mut Value) {
    crate::migration::transform_v14_to_v15_value(envelope).expect("transform must not error");
}
