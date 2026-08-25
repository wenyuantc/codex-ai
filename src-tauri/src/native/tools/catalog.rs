use serde_json::json;

use crate::native::model::types::ToolSpec;

pub const READ_ONLY_NATIVE_TOOL_NAMES: &[&str] = &[
    "Read",
    "Glob",
    "Grep",
    "TodoRead",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
];

pub fn is_read_only_native_tool(name: &str) -> bool {
    matches!(
        name,
        "Read" | "Glob" | "Grep" | "TodoRead" | "TodoWrite" | "WebFetch" | "WebSearch"
    )
}

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "Read",
            "Read a file from the workspace. Prefer this over cat in Bash.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "offset": {"type": "integer"},
                    "limit": {"type": "integer"}
                },
                "required": ["file_path"]
            }),
        ),
        spec(
            "Write",
            "Create or overwrite a file. Read an existing file before overwriting it.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["file_path", "content"]
            }),
        ),
        spec(
            "Edit",
            "Exact string replacement in a file. Read the file first. old_string must be unique unless replace_all is true.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"},
                    "replace_all": {"type": "boolean"}
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        ),
        spec(
            "Bash",
            "Run a shell command in the workspace. Prefer Read/Glob/Grep for file inspection.",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout": {"type": "integer"},
                    "description": {"type": "string"}
                },
                "required": ["command"]
            }),
        ),
        spec(
            "Glob",
            "Find files by glob pattern, such as **/*.rs.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"}
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "Grep",
            "Search file contents. Prefer this over grep/rg in Bash.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "glob": {"type": "string"},
                    "head_limit": {"type": "integer"}
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "TodoRead",
            "Read the current session todo list.",
            json!({"type": "object", "properties": {}}),
        ),
        spec(
            "TodoWrite",
            "Replace the session todo list. Keep at most one item in_progress.",
            json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "content": {"type": "string"},
                                "status": {"type": "string"},
                                "priority": {"type": "string"}
                            },
                            "required": ["content", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        ),
        spec(
            "WebFetch",
            "Fetch a public http(s) URL, convert readable content to text, and optionally extract by prompt.",
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "prompt": {"type": "string"}
                },
                "required": ["url"]
            }),
        ),
        spec(
            "WebSearch",
            "Search the live web. Prefer a natural-language query. After answering, list Sources as markdown links.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "num_results": {"type": "integer"}
                },
                "required": ["query"]
            }),
        ),
        spec(
            "Agent",
            "Delegate a self-contained subtask to a child agent. Multiple calls in one turn run in parallel up to the session cap (see Max concurrent sub-agents). The child does not see this conversation. Use explore for read-only research and general for edits. Do not use this for a single Read or Grep.",
            json!({
                "type": "object",
                "properties": {
                    "description": {"type": "string"},
                    "prompt": {"type": "string"},
                    "subagent_type": {"type": "string"}
                },
                "required": ["description", "prompt"]
            }),
        ),
    ]
}

fn spec(name: &str, description: &str, parameters: serde_json::Value) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_tools_exclude_writers() {
        assert!(is_read_only_native_tool("Read"));
        assert!(is_read_only_native_tool("Grep"));
        assert!(is_read_only_native_tool("TodoWrite"));
        assert!(!is_read_only_native_tool("Write"));
        assert!(!is_read_only_native_tool("Edit"));
        assert!(!is_read_only_native_tool("Bash"));
        assert!(!is_read_only_native_tool("Agent"));
    }

    #[test]
    fn includes_core_tools() {
        let names: Vec<_> = tool_specs().into_iter().map(|item| item.name).collect();
        for expected in [
            "Read",
            "Write",
            "Edit",
            "Bash",
            "Glob",
            "Grep",
            "TodoRead",
            "TodoWrite",
            "WebFetch",
            "WebSearch",
            "Agent",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }
}
