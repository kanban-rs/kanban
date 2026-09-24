pub const DECLINED_CAPABILITIES: &[(&str, &str)] = &[
    (
        "undo",
        "undo: the CLI opens a fresh context per invocation, so the per-process undo history is always empty",
    ),
    (
        "redo",
        "redo: the CLI opens a fresh context per invocation, so the per-process undo history is always empty",
    ),
];
