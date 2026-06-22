use crate::context::McpContext;
use crate::helpers::error_mapping::kanban_err_to_mcp;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;
use tokio::sync::Mutex;

// ---------- Locked sessions ----------
//
// Two flavours, named by intent. Each acquires the context lock and drops it
// when the closure returns; resolution + the work share one consistent view
// of state, closing any TOCTOU window.
//
// - `locked_read(ctx, |ctx| ...)` — lock, run closure, drop. No disk reload,
//   no save. The closure takes `&McpContext` so the type system enforces
//   read-only semantics. Use for tool handlers that resolve names + read
//   state without mutating.
//
// - `locked_write(ctx, |ctx| ...)` — lock, reload from disk, run closure,
//   save, drop. The closure takes `&mut McpContext`. Reload+save bracket
//   the closure so mutations see the latest disk state and are persisted.
//
// For trivial reads with no resolution (`tool_list_boards`, etc.) the older
// `read_op!` macro is still appropriate — it's a one-liner that elides the
// closure ceremony.

/// Acquire the context lock and run the closure with read-only access.
///
/// The in-memory cache is **not** reloaded — reads are served from whatever
/// state the previous tool call left behind. If a separate process wrote to
/// the file since the last reload, the read may be stale. That's an
/// intentional perf tradeoff: typical MCP usage is single-process, and the
/// reload cost (file read + parse) is significant relative to the read
/// itself.
pub(crate) async fn locked_read<T, E, F>(ctx: &Arc<Mutex<McpContext>>, f: F) -> Result<T, McpError>
where
    F: FnOnce(&McpContext) -> Result<T, E>,
    E: Into<McpError>,
{
    let guard = ctx.lock().await;
    f(&guard).map_err(Into::into)
}

/// Acquire the context lock, reload from disk, run the closure with mutable
/// access, then save and drop. Reload + save bracket the closure so the
/// mutation always operates on the latest disk state and is persisted before
/// the lock releases.
///
/// # Reload semantics and undo limitations
///
/// `guard.reload()` fully discards the in-memory cache and resets undo
/// history to the current on-disk state. As a consequence, within-session
/// undo history from earlier tool calls is always wiped before each
/// mutation: `tool_undo` can only undo the operation recorded during the
/// **current** tool call, not operations from prior calls.
///
/// **Future work**: a `reload_if_changed()` method that compares file
/// metadata (mtime / instance_id) and skips the full reload when no
/// external write has occurred would let undo history persist across calls
/// in the same session. Track as `KanbanBackend::reload_if_changed()`.
pub(crate) async fn locked_write<T, E, F>(ctx: &Arc<Mutex<McpContext>>, f: F) -> Result<T, McpError>
where
    F: FnOnce(&mut McpContext) -> Result<T, E>,
    E: Into<McpError>,
{
    let mut guard = ctx.lock().await;
    guard.reload().await.map_err(kanban_err_to_mcp)?;
    let result = f(&mut guard).map_err(Into::into)?;
    guard.save().await.map_err(kanban_err_to_mcp)?;
    Ok(result)
}

/// Lock the context, reload from disk, execute a mutating operation, then save.
///
/// # Reload semantics and undo limitations
///
/// Every invocation begins with `guard.reload()`, which fully discards the
/// in-memory cache and resets undo history to the current on-disk state.
/// Consequently, within-session undo history from earlier API calls is always
/// wiped before each mutation: `tool_undo` can only undo the operation
/// recorded during the **current** tool call, not operations from prior calls.
///
/// **Future work**: a `reload_if_changed()` method that compares file metadata
/// (mtime / instance_id) and skips the full reload when no external write has
/// occurred would allow undo history to persist across calls in the same
/// session. Track as `KanbanBackend::reload_if_changed()`.
macro_rules! mutating_op {
    ($ctx:expr, $method:ident $(, $arg:expr)*) => {{
        async {
            let mut guard = $ctx.lock().await;
            guard.reload().await.map_err($crate::helpers::kanban_err_to_mcp)?;
            let result = guard.$method($($arg),*).map_err($crate::helpers::kanban_err_to_mcp)?;
            guard.save().await.map_err($crate::helpers::kanban_err_to_mcp)?;
            Ok::<_, rmcp::model::ErrorData>(result)
        }
        .await
    }};
}

/// Lock, read (no save).
macro_rules! read_op {
    ($ctx:expr, $method:ident $(, $arg:expr)*) => {{
        $ctx.lock().await.$method($($arg),*).map_err($crate::helpers::kanban_err_to_mcp)
    }};
}

pub(crate) use mutating_op;
pub(crate) use read_op;
