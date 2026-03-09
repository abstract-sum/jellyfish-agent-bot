use std::fs;
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use walkdir::WalkDir;

use openclaw_core::{AppError, AppResult};

use crate::traits::{Tool, ToolDefinition, ToolOutput};

fn ensure_relative_path(path: &Path) -> AppResult<()> {
    if path.is_absolute() {
        return Err(AppError::Tool("absolute paths are not allowed".to_string()));
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::Tool(
            "parent directory traversal is not allowed".to_string(),
        ));
    }

    Ok(())
}

fn resolve_workspace_path(workspace_root: &Path, relative: &str) -> AppResult<PathBuf> {
    let relative_path = Path::new(relative);
    ensure_relative_path(relative_path)?;
    Ok(workspace_root.join(relative_path))
}

#[derive(Debug, Clone)]
pub struct ReadTool {
    workspace_root: PathBuf,
}

impl ReadTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct ReadArgs {
    path: String,
}

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read a UTF-8 text file from the workspace".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative path to a file"}
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, input: Value) -> AppResult<ToolOutput> {
        let args: ReadArgs = serde_json::from_value(input)?;
        let path = resolve_workspace_path(&self.workspace_root, &args.path)?;
        let content = fs::read_to_string(&path)?;

        Ok(ToolOutput {
            content: format!("FILE: {}\n{}", args.path, content),
        })
    }
}

#[derive(Debug, Clone)]
pub struct GlobTool {
    workspace_root: PathBuf,
}

impl GlobTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct GlobArgs {
    pattern: String,
}

#[async_trait]
impl Tool for GlobTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: "List files in the workspace using a glob pattern".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "A glob like src/**/*.rs"}
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, input: Value) -> AppResult<ToolOutput> {
        let args: GlobArgs = serde_json::from_value(input)?;
        let pattern = self
            .workspace_root
            .join(&args.pattern)
            .to_string_lossy()
            .to_string();

        let mut matches = glob::glob(&pattern)
            .map_err(|error| AppError::Tool(error.to_string()))?
            .filter_map(Result::ok)
            .filter_map(|path| path.strip_prefix(&self.workspace_root).ok().map(PathBuf::from))
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        matches.sort();

        Ok(ToolOutput {
            content: if matches.is_empty() {
                format!("No files matched pattern {}", args.pattern)
            } else {
                format!("Matches for {}:\n{}", args.pattern, matches.join("\n"))
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct GrepTool {
    workspace_root: PathBuf,
}

impl GrepTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
}

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: "Search workspace text files with a regular expression".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex to search for"}
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, input: Value) -> AppResult<ToolOutput> {
        let args: GrepArgs = serde_json::from_value(input)?;
        let regex = Regex::new(&args.pattern).map_err(|error| AppError::Tool(error.to_string()))?;
        let mut matches = Vec::new();

        for entry in WalkDir::new(&self.workspace_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let Ok(relative) = entry.path().strip_prefix(&self.workspace_root) else {
                continue;
            };

            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };

            for (index, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(format!("{}:{}:{}", relative.display(), index + 1, line.trim()));

                    if matches.len() >= 50 {
                        break;
                    }
                }
            }

            if matches.len() >= 50 {
                break;
            }
        }

        Ok(ToolOutput {
            content: if matches.is_empty() {
                format!("No matches found for pattern {}", args.pattern)
            } else {
                format!("Matches for {}:\n{}", args.pattern, matches.join("\n"))
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_workspace() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let unique = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("openclaw-tools-{suffix}-{unique}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[tokio::test]
    async fn read_tool_reads_relative_file() {
        let workspace = temp_workspace();
        fs::write(workspace.join("sample.txt"), "hello").unwrap();

        let tool = ReadTool::new(workspace.clone());
        let output = tool.call(json!({ "path": "sample.txt" })).await.unwrap();

        assert!(output.content.contains("hello"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn glob_tool_lists_matches() {
        let workspace = temp_workspace();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(workspace.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();

        let tool = GlobTool::new(workspace.clone());
        let output = tool.call(json!({ "pattern": "src/*.rs" })).await.unwrap();

        assert!(output.content.contains("src/lib.rs"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn grep_tool_finds_matching_lines() {
        let workspace = temp_workspace();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(workspace.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();

        let tool = GrepTool::new(workspace.clone());
        let output = tool.call(json!({ "pattern": "demo" })).await.unwrap();

        assert!(output.content.contains("src/lib.rs:1"));
        fs::remove_dir_all(workspace).unwrap();
    }
}
