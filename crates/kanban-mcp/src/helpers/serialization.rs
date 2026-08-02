use rmcp::model::{CallToolResult, Content, ErrorData as McpError};

pub(crate) fn to_call_tool_result<T: serde::Serialize>(
    value: &T,
) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| McpError::internal_error(format!("Serialization failed: {}", e), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

pub(crate) fn to_call_tool_result_json(
    value: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(&value)
        .map_err(|e| McpError::internal_error(format!("Serialization failed: {}", e), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_call_tool_result_serializes_struct() {
        use rmcp::model::RawContent;
        #[derive(serde::Serialize)]
        struct Foo {
            x: i32,
        }
        let result = to_call_tool_result(&Foo { x: 42 }).unwrap();
        match &result.content[0].raw {
            RawContent::Text(t) => assert!(t.text.contains("42")),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn to_call_tool_result_json_serializes_value() {
        use rmcp::model::RawContent;
        let val = serde_json::json!({"key": "value"});
        let result = to_call_tool_result_json(val).unwrap();
        match &result.content[0].raw {
            RawContent::Text(t) => {
                assert!(t.text.contains("key"));
                assert!(t.text.contains("value"));
            }
            _ => panic!("Expected text content"),
        }
    }
}
