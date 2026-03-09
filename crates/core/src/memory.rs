use serde::{Deserialize, Serialize};
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
}

impl MemoryEntry {
    pub fn new(kind: MemoryKind, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            content: content.into(),
        }
    }
}
