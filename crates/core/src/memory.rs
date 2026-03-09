use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryKind {
    Preference,
    Profile,
    Task,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserPreference {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UserProfile {
    pub display_name: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub preferences: Vec<UserPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub kind: MemoryKind,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
}

impl MemoryEntry {
    pub fn new(kind: MemoryKind, content: impl Into<String>) -> Self {
        let now = unix_timestamp();
        Self {
            id: Uuid::new_v4(),
            kind,
            content: content.into(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = unix_timestamp();
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
