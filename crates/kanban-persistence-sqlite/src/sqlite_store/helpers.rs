use chrono::{DateTime, Utc};
use kanban_domain::{KanbanError, KanbanResult};
use uuid::Uuid;

pub(crate) fn run<F: std::future::Future<Output = T>, T>(f: F) -> T {
    let handle = tokio::runtime::Handle::current();
    debug_assert!(
        handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
        "SqliteStore requires a multi-threaded Tokio runtime (e.g. #[tokio::main] or \
         tokio::runtime::Runtime::new()). The current_thread runtime is not supported \
         because synchronous DataStore methods need to block on async SQLite I/O."
    );
    tokio::task::block_in_place(|| handle.block_on(f))
}

pub(crate) fn db_err(e: sqlx::Error) -> KanbanError {
    KanbanError::Database(e.to_string())
}

pub(crate) fn ser_err(msg: impl std::fmt::Display) -> KanbanError {
    KanbanError::Serialization(msg.to_string())
}

pub(crate) fn p_uuid(s: &str) -> KanbanResult<Uuid> {
    Uuid::parse_str(s).map_err(ser_err)
}

pub(crate) fn p_dt(s: &str) -> KanbanResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map_err(ser_err)
        .map(|dt| dt.with_timezone(&Utc))
}

pub(crate) fn p_enum<T: serde::de::DeserializeOwned>(s: &str, label: &str) -> KanbanResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|_| ser_err(format!("unknown {label} variant: {s}")))
}

/// Render a unit-variant enum to its serde wire name (e.g.
/// `CardEdgeType::Spawns -> "ParentOf"`). Symmetric with [`p_enum`].
/// Avoids coupling the on-disk format to `#[derive(Debug)]`, which is
/// easy to customize and would break the read/write round-trip silently.
pub(crate) fn ser_enum<T: serde::Serialize + std::fmt::Debug>(
    v: &T,
    label: &str,
) -> KanbanResult<String> {
    match serde_json::to_value(v).map_err(ser_err)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(ser_err(format!(
            "{label} did not serialise to a JSON string: {other}"
        ))),
    }
}

pub(crate) fn fmt_dt(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

pub(crate) fn required_str<'a>(value: &'a str, field: &str) -> KanbanResult<&'a str> {
    if value.is_empty() {
        Err(ser_err(format!(
            "required field '{field}' must not be empty"
        )))
    } else {
        Ok(value)
    }
}

pub(crate) fn opt_dt(dt: &Option<DateTime<Utc>>) -> Option<String> {
    dt.as_ref().map(fmt_dt)
}
