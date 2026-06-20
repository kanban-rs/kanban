//! Shared wire-to-domain conversion helpers.

use kanban_domain::FieldUpdate;

/// Create-path mapping of an optional field to a domain [`FieldUpdate`]:
/// a present value becomes `Set`, an absent one stays `NoChange` (a create never
/// clears).
///
/// Deliberately distinct from `FieldUpdate::from(Option<T>)`, which maps `None`
/// to `Clear` and is correct only for the PUT full-replace path.
pub(crate) fn set_or_no_change<T>(value: Option<T>) -> FieldUpdate<T> {
    match value {
        Some(v) => FieldUpdate::Set(v),
        None => FieldUpdate::NoChange,
    }
}
