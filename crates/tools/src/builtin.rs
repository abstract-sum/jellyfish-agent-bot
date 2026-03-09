use std::fs;
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use walkdir::WalkDir;

use jellyfish_core::{AppError, AppResult};

use crate::traits::{Tool, ToolDefinition, ToolOutput};

const JELLYFISH_DIR: &str = ".jellyfish";
const NOTES_FILE: &str = "notes.json";
const TODOS_FILE: &str = "todos.json";

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

fn jellyfish_data_path(workspace_root: &Path, file_name: &str) -> PathBuf {
    workspace_root.join(JELLYFISH_DIR).join(file_name)
}

fn ensure_data_parent(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
struct StoredNote {
    title: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
struct StoredTodo {
    text: String,
    done: bool,
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

#[derive(Debug, Clone)]
pub struct NoteTool {
    workspace_root: PathBuf,
}

impl NoteTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct NoteArgs {
    action: String,
    title: Option<String>,
    content: Option<String>,
}

#[async_trait]
impl Tool for NoteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "notes".to_string(),
            description: "Save or list personal notes stored in Jellyfish local state".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "list or add"},
                    "title": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["action"]
            }),
        }
    }

    async fn call(&self, input: Value) -> AppResult<ToolOutput> {
        let args: NoteArgs = serde_json::from_value(input)?;
        let path = jellyfish_data_path(&self.workspace_root, NOTES_FILE);
        let mut notes = load_json_or_default::<Vec<StoredNote>>(&path)?;

        match args.action.as_str() {
            "list" => Ok(ToolOutput {
                content: if notes.is_empty() {
                    "No notes saved yet".to_string()
                } else {
                    notes
                        .iter()
                        .enumerate()
                        .map(|(index, note)| format!("{}. {}: {}", index + 1, note.title, note.content))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            }),
            "add" => {
                let title = args
                    .title
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| AppError::Tool("notes.add requires title".to_string()))?;
                let content = args
                    .content
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| AppError::Tool("notes.add requires content".to_string()))?;

                if let Some(existing) = notes.iter_mut().find(|note| note.title == title) {
                    existing.content = content.clone();
                } else {
                    notes.push(StoredNote {
                        title: title.clone(),
                        content: content.clone(),
                    });
                }

                save_json(&path, &notes)?;
                Ok(ToolOutput {
                    content: format!("Saved note '{}'", title),
                })
            }
            other => Err(AppError::Tool(format!("unsupported notes action: {other}"))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TodoTool {
    workspace_root: PathBuf,
}

impl TodoTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct TodoArgs {
    action: String,
    text: Option<String>,
    index: Option<usize>,
}

#[async_trait]
impl Tool for TodoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "todos".to_string(),
            description: "Manage a simple personal todo list stored in Jellyfish local state".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "list, add, or done"},
                    "text": {"type": "string"},
                    "index": {"type": "integer"}
                },
                "required": ["action"]
            }),
        }
    }

    async fn call(&self, input: Value) -> AppResult<ToolOutput> {
        let args: TodoArgs = serde_json::from_value(input)?;
        let path = jellyfish_data_path(&self.workspace_root, TODOS_FILE);
        let mut todos = load_json_or_default::<Vec<StoredTodo>>(&path)?;

        match args.action.as_str() {
            "list" => Ok(ToolOutput {
                content: if todos.is_empty() {
                    "No todos saved yet".to_string()
                } else {
                    todos
                        .iter()
                        .enumerate()
                        .map(|(index, todo)| {
                            let state = if todo.done { "done" } else { "open" };
                            format!("{}. [{}] {}", index + 1, state, todo.text)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            }),
            "add" => {
                let text = args
                    .text
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| AppError::Tool("todos.add requires text".to_string()))?;
                todos.push(StoredTodo {
                    text: text.clone(),
                    done: false,
                });
                save_json(&path, &todos)?;
                Ok(ToolOutput {
                    content: format!("Added todo '{}'", text),
                })
            }
            "done" => {
                let index = args
                    .index
                    .ok_or_else(|| AppError::Tool("todos.done requires index".to_string()))?;
                let todo = todos
                    .get_mut(index.saturating_sub(1))
                    .ok_or_else(|| AppError::Tool(format!("todo index out of range: {index}")))?;
                todo.done = true;
                let text = todo.text.clone();
                save_json(&path, &todos)?;
                Ok(ToolOutput {
                    content: format!("Completed todo '{}'", text),
                })
            }
            other => Err(AppError::Tool(format!("unsupported todos action: {other}"))),
        }
    }
}

fn load_json_or_default<T>(path: &Path) -> AppResult<T>
where
    T: Default + for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(T::default());
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(T::default());
    }

    Ok(serde_json::from_str(&content)?)
}

fn save_json<T>(path: &Path, value: &T) -> AppResult<()>
where
    T: serde::Serialize,
{
    ensure_data_parent(path)?;
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ApplyPatchTool {
    workspace_root: PathBuf,
}

impl ApplyPatchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct ApplyPatchArgs {
    patch: String,
}

#[derive(Debug)]
enum PatchOperation {
    Add {
        path: String,
        lines: Vec<String>,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        move_to: Option<String>,
        hunks: Vec<PatchHunk>,
    },
}

#[derive(Debug)]
struct PatchHunk {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Apply a structured patch to files inside the workspace".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "Patch text using the simplified *** Begin Patch / *** End Patch format"
                    }
                },
                "required": ["patch"]
            }),
        }
    }

    async fn call(&self, input: Value) -> AppResult<ToolOutput> {
        let args: ApplyPatchArgs = serde_json::from_value(input)?;
        let operations = parse_patch(&args.patch)?;
        let mut results = Vec::new();

        for operation in operations {
            match operation {
                PatchOperation::Add { path, lines } => {
                    let resolved = resolve_workspace_path(&self.workspace_root, &path)?;
                    if resolved.exists() {
                        return Err(AppError::Tool(format!("file already exists: {path}")));
                    }

                    if let Some(parent) = resolved.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    let content = join_lines(&lines);
                    fs::write(&resolved, content)?;
                    results.push(format!("added {path}"));
                }
                PatchOperation::Delete { path } => {
                    let resolved = resolve_workspace_path(&self.workspace_root, &path)?;
                    if !resolved.exists() {
                        return Err(AppError::Tool(format!("file does not exist: {path}")));
                    }

                    fs::remove_file(&resolved)?;
                    results.push(format!("deleted {path}"));
                }
                PatchOperation::Update {
                    path,
                    move_to,
                    hunks,
                } => {
                    let resolved = resolve_workspace_path(&self.workspace_root, &path)?;
                    let original = fs::read_to_string(&resolved)?;
                    let mut current_lines = split_lines_preserve_newlines(&original);

                    for hunk in hunks {
                        apply_hunk(&mut current_lines, &hunk, &path)?;
                    }

                    let updated_path = move_to.unwrap_or(path);
                    let resolved_updated = resolve_workspace_path(&self.workspace_root, &updated_path)?;
                    if let Some(parent) = resolved_updated.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::write(&resolved_updated, join_lines(&current_lines))?;
                    if resolved != resolved_updated {
                        fs::remove_file(&resolved)?;
                    }

                    results.push(format!("updated {updated_path}"));
                }
            }
        }

        Ok(ToolOutput {
            content: results.join("\n"),
        })
    }
}

fn join_lines(lines: &[String]) -> String {
    lines.concat()
}

fn split_lines_preserve_newlines(content: &str) -> Vec<String> {
    if content.is_empty() {
        return Vec::new();
    }

    content
        .split_inclusive('\n')
        .map(ToString::to_string)
        .collect()
}

fn parse_patch(patch: &str) -> AppResult<Vec<PatchOperation>> {
    let lines = patch.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some("*** Begin Patch") {
        return Err(AppError::Tool(
            "patch must start with *** Begin Patch".to_string(),
        ));
    }
    if lines.last().copied() != Some("*** End Patch") {
        return Err(AppError::Tool(
            "patch must end with *** End Patch".to_string(),
        ));
    }

    let mut operations = Vec::new();
    let mut index = 1;

    while index < lines.len() - 1 {
        let line = lines[index];
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            index += 1;
            let mut add_lines = Vec::new();
            while index < lines.len() - 1 && !lines[index].starts_with("*** ") {
                let body = lines[index]
                    .strip_prefix('+')
                    .ok_or_else(|| AppError::Tool("add file lines must start with +".to_string()))?;
                add_lines.push(format!("{body}\n"));
                index += 1;
            }
            operations.push(PatchOperation::Add {
                path: path.to_string(),
                lines: add_lines,
            });
            continue;
        }

        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            operations.push(PatchOperation::Delete {
                path: path.to_string(),
            });
            index += 1;
            continue;
        }

        if let Some(path) = line.strip_prefix("*** Update File: ") {
            index += 1;
            let mut move_to = None;
            if index < lines.len() - 1 {
                if let Some(target) = lines[index].strip_prefix("*** Move to: ") {
                    move_to = Some(target.to_string());
                    index += 1;
                }
            }

            let mut hunks = Vec::new();
            while index < lines.len() - 1 && !lines[index].starts_with("*** ") {
                if !lines[index].starts_with("@@") {
                    return Err(AppError::Tool(format!(
                        "expected hunk header in update for {path}"
                    )));
                }

                index += 1;
                let mut old_lines = Vec::new();
                let mut new_lines = Vec::new();
                while index < lines.len() - 1
                    && !lines[index].starts_with("@@")
                    && !lines[index].starts_with("*** ")
                {
                    let hunk_line = lines[index];
                    let (prefix, body) = hunk_line.split_at(1);
                    match prefix {
                        " " => {
                            old_lines.push(format!("{body}\n"));
                            new_lines.push(format!("{body}\n"));
                        }
                        "-" => old_lines.push(format!("{body}\n")),
                        "+" => new_lines.push(format!("{body}\n")),
                        _ => {
                            return Err(AppError::Tool(format!(
                                "unsupported hunk line in update for {path}: {hunk_line}"
                            )))
                        }
                    }
                    index += 1;
                }

                hunks.push(PatchHunk { old_lines, new_lines });
            }

            operations.push(PatchOperation::Update {
                path: path.to_string(),
                move_to,
                hunks,
            });
            continue;
        }

        return Err(AppError::Tool(format!("unsupported patch header: {line}")));
    }

    Ok(operations)
}

fn apply_hunk(lines: &mut Vec<String>, hunk: &PatchHunk, path: &str) -> AppResult<()> {
    if hunk.old_lines.is_empty() {
        lines.extend(hunk.new_lines.clone());
        return Ok(());
    }

    let old_len = hunk.old_lines.len();
    let mut start = None;

    for index in 0..=lines.len().saturating_sub(old_len) {
        if lines[index..index + old_len] == hunk.old_lines[..] {
            start = Some(index);
            break;
        }
    }

    let start = start.ok_or_else(|| {
        AppError::Tool(format!("failed to match update hunk in file: {path}"))
    })?;

    lines.splice(start..start + old_len, hunk.new_lines.clone());
    Ok(())
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
        let root = std::env::temp_dir().join(format!("jellyfish-tools-{suffix}-{unique}"));
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

    #[tokio::test]
    async fn apply_patch_tool_updates_existing_file() {
        let workspace = temp_workspace();
        fs::write(workspace.join("sample.txt"), "hello\nworld\n").unwrap();

        let tool = ApplyPatchTool::new(workspace.clone());
        let output = tool
            .call(json!({
                "patch": "*** Begin Patch\n*** Update File: sample.txt\n@@\n-hello\n+hi\n world\n*** End Patch"
            }))
            .await
            .unwrap();

        assert!(output.content.contains("updated sample.txt"));
        let updated = fs::read_to_string(workspace.join("sample.txt")).unwrap();
        assert_eq!(updated, "hi\nworld\n");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn apply_patch_tool_adds_new_file() {
        let workspace = temp_workspace();

        let tool = ApplyPatchTool::new(workspace.clone());
        tool.call(json!({
            "patch": "*** Begin Patch\n*** Add File: src/new.rs\n+pub fn created() {}\n*** End Patch"
        }))
        .await
        .unwrap();

        let created = fs::read_to_string(workspace.join("src/new.rs")).unwrap();
        assert_eq!(created, "pub fn created() {}\n");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn notes_tool_saves_and_lists_notes() {
        let workspace = temp_workspace();
        let tool = NoteTool::new(workspace.clone());

        tool.call(json!({
            "action": "add",
            "title": "morning",
            "content": "prepare summary"
        }))
        .await
        .unwrap();

        let output = tool.call(json!({ "action": "list" })).await.unwrap();
        assert!(output.content.contains("morning"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn todos_tool_tracks_completion() {
        let workspace = temp_workspace();
        let tool = TodoTool::new(workspace.clone());

        tool.call(json!({
            "action": "add",
            "text": "review priorities"
        }))
        .await
        .unwrap();
        tool.call(json!({
            "action": "done",
            "index": 1
        }))
        .await
        .unwrap();

        let output = tool.call(json!({ "action": "list" })).await.unwrap();
        assert!(output.content.contains("[done]"));
        fs::remove_dir_all(workspace).unwrap();
    }
}
