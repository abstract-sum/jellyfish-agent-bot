use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use jellyfish_core::{MessageRole, Session};

const JELLYFISH_DIR: &str = ".jellyfish";
const NOTES_FILE: &str = "notes.json";
const TODOS_FILE: &str = "todos.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalHit {
    pub source: String,
    pub content: String,
    pub score: usize,
}

#[derive(Debug, Default)]
pub struct RetrievalSnapshot {
    entries: Vec<RetrievalEntry>,
}

#[derive(Debug, Clone)]
struct RetrievalEntry {
    source: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct StoredNote {
    title: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct StoredTodo {
    text: String,
    done: bool,
}

impl RetrievalSnapshot {
    pub fn load(workspace_root: &Path, session: &Session) -> Result<Self> {
        let mut entries = Vec::new();

        if let Some(display_name) = &session.profile.display_name {
            entries.push(RetrievalEntry {
                source: "profile".to_string(),
                content: format!("display_name={display_name}"),
            });
        }
        if let Some(locale) = &session.profile.locale {
            entries.push(RetrievalEntry {
                source: "profile".to_string(),
                content: format!("locale={locale}"),
            });
        }
        if let Some(timezone) = &session.profile.timezone {
            entries.push(RetrievalEntry {
                source: "profile".to_string(),
                content: format!("timezone={timezone}"),
            });
        }
        for preference in &session.profile.preferences {
            entries.push(RetrievalEntry {
                source: "preference".to_string(),
                content: format!("{}={}", preference.key, preference.value),
            });
        }
        for memory in &session.memories {
            entries.push(RetrievalEntry {
                source: format!("memory::{:?}", memory.kind),
                content: memory.content.clone(),
            });
        }
        for message in session.messages.iter().rev().take(12) {
            if matches!(message.role, MessageRole::User | MessageRole::Assistant) {
                entries.push(RetrievalEntry {
                    source: format!("conversation::{:?}", message.role),
                    content: message.content.clone(),
                });
            }
        }
        for note in
            load_json_or_default::<Vec<StoredNote>>(&state_path(workspace_root, NOTES_FILE))?
        {
            entries.push(RetrievalEntry {
                source: "notes".to_string(),
                content: format!("{}: {}", note.title, note.content),
            });
        }
        for todo in
            load_json_or_default::<Vec<StoredTodo>>(&state_path(workspace_root, TODOS_FILE))?
        {
            let status = if todo.done { "done" } else { "open" };
            entries.push(RetrievalEntry {
                source: "todos".to_string(),
                content: format!("[{status}] {}", todo.text),
            });
        }

        Ok(Self { entries })
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<RetrievalHit> {
        let query_tokens = tokenize(query);
        let compact_query = compact(query);
        let query_bigrams = bigrams(&compact_query);
        let mut hits = self
            .entries
            .iter()
            .map(|entry| {
                let tokens = tokenize(&entry.content);
                let token_score = tokens
                    .iter()
                    .filter(|token| query_tokens.contains(token))
                    .count();
                let compact_content = compact(&entry.content);
                let substring_score =
                    if !compact_query.is_empty() && compact_content.contains(&compact_query) {
                        2
                    } else {
                        0
                    };
                let bigram_score = if query_bigrams.is_empty() {
                    0
                } else {
                    bigrams(&compact_content)
                        .into_iter()
                        .filter(|gram| query_bigrams.contains(gram))
                        .count()
                };
                (entry, token_score + substring_score + bigram_score)
            })
            .filter(|(_, score)| *score > 0)
            .map(|(entry, score)| RetrievalHit {
                source: entry.source.clone(),
                content: entry.content.clone(),
                score,
            })
            .collect::<Vec<_>>();

        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(left.source.cmp(&right.source))
                .then(left.content.cmp(&right.content))
        });
        hits.truncate(limit);
        hits
    }

    pub fn context_lines(&self, query: &str, limit: usize) -> Vec<String> {
        self.search(query, limit)
            .into_iter()
            .map(|hit| format!("{}: {}", hit.source, hit.content))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn load_json_or_default<T>(path: &Path) -> Result<T>
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

fn state_path(workspace_root: &Path, file_name: &str) -> PathBuf {
    workspace_root.join(JELLYFISH_DIR).join(file_name)
}

fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|char: char| !char.is_alphanumeric() && char != '_' && char != '-')
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn compact(value: &str) -> String {
    value.chars().filter(|char| !char.is_whitespace()).collect()
}

fn bigrams(value: &str) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() < 2 {
        return Vec::new();
    }

    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_workspace() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("jellyfish-retrieval-{suffix}"));
        fs::create_dir_all(root.join(JELLYFISH_DIR)).unwrap();
        root
    }

    #[test]
    fn retrieval_finds_profile_and_memory_context() {
        let workspace = temp_workspace();
        let mut session = Session::new();
        session.set_display_name("Yvonne");
        session.remember(
            jellyfish_core::MemoryKind::Note,
            "weekly review every sunday",
        );

        let snapshot = RetrievalSnapshot::load(&workspace, &session).unwrap();
        let hits = snapshot.search("sunday review", 5);

        assert!(!hits.is_empty());
        fs::remove_dir_all(workspace).unwrap();
    }
}
