use serde::{Deserialize, Serialize};

use crate::{
    event::AgentEvent,
    memory::{MemoryEntry, MemoryKind, UserPreference, UserProfile},
    types::SessionId,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub profile: UserProfile,
    pub memories: Vec<MemoryEntry>,
    pub messages: Vec<Message>,
    pub events: Vec<AgentEvent>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: SessionId::new(),
            profile: UserProfile::default(),
            memories: Vec::new(),
            messages: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn push_message(&mut self, role: MessageRole, content: impl Into<String>) {
        self.messages.push(Message {
            role,
            content: content.into(),
        });
    }

    pub fn push_event(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    pub fn remember(&mut self, kind: MemoryKind, content: impl Into<String>) {
        self.remember_unique(kind, content);
    }

    pub fn remember_unique(&mut self, kind: MemoryKind, content: impl Into<String>) {
        let content = content.into();

        if let Some(existing) = self
            .memories
            .iter_mut()
            .find(|entry| entry.kind == kind && entry.content == content)
        {
            existing.touch();
        } else {
            self.memories.push(MemoryEntry::new(kind, content));
        }
    }

    pub fn set_display_name(&mut self, display_name: impl Into<String>) {
        self.profile.display_name = Some(display_name.into());
    }

    pub fn set_timezone(&mut self, timezone: impl Into<String>) {
        self.profile.timezone = Some(timezone.into());
    }

    pub fn set_locale(&mut self, locale: impl Into<String>) {
        self.profile.locale = Some(locale.into());
    }

    pub fn set_preference(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();

        if let Some(existing) = self
            .profile
            .preferences
            .iter_mut()
            .find(|entry| entry.key == key)
        {
            existing.value = value;
        } else {
            self.profile.preferences.push(UserPreference { key, value });
        }
    }

    pub fn memory_summary(&self, limit: usize) -> Vec<String> {
        self.memories
            .iter()
            .rev()
            .take(limit)
            .map(|entry| format!("{:?}: {}", entry.kind, entry.content))
            .collect()
    }

    pub fn relevant_memories(&self, query: &str, limit: usize) -> Vec<String> {
        let query_tokens = tokenize(query);
        let mut ranked = self
            .memories
            .iter()
            .map(|entry| {
                let overlap = tokenize(&entry.content)
                    .into_iter()
                    .filter(|token| query_tokens.contains(token))
                    .count();
                let recency = entry.updated_at;
                (entry, overlap, recency)
            })
            .filter(|(_, overlap, _)| *overlap > 0)
            .collect::<Vec<_>>();

        ranked.sort_by(|left, right| right.1.cmp(&left.1).then(right.2.cmp(&left.2)));

        let mut selected = ranked
            .into_iter()
            .take(limit)
            .map(|(entry, _, _)| format!("{:?}: {}", entry.kind, entry.content))
            .collect::<Vec<_>>();

        if selected.len() < limit {
            for fallback in self.memory_summary(limit).into_iter() {
                if selected.len() >= limit {
                    break;
                }
                if !selected.contains(&fallback) {
                    selected.push(fallback);
                }
            }
        }

        selected
    }
}

fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|char: char| !char.is_alphanumeric() && char != '_' && char != '-')
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_tracks_profile_and_memory() {
        let mut session = Session::new();
        session.set_display_name("Yvonne");
        session.set_preference("tone", "concise");
        session.remember(MemoryKind::Note, "prefers morning summaries");

        assert_eq!(session.profile.display_name.as_deref(), Some("Yvonne"));
        assert_eq!(session.profile.preferences.len(), 1);
        assert_eq!(session.memory_summary(1).len(), 1);
    }

    #[test]
    fn relevant_memories_prefers_matches() {
        let mut session = Session::new();
        session.remember(MemoryKind::Note, "likes concise morning summaries");
        session.remember(MemoryKind::Note, "prefers travel planning checklists");

        let relevant = session.relevant_memories("Can you make a morning summary?", 2);
        assert!(relevant[0].contains("morning summaries"));
    }
}
