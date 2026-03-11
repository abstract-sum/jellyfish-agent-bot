use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};

const DEDUP_TTL_SECS: u64 = 30 * 60;
const DEDUP_MAX_SIZE: usize = 1_000;

#[derive(Debug, Default, Serialize, Deserialize)]
struct DedupFile {
    entries: HashMap<String, u64>,
}

#[derive(Debug, Default)]
pub struct DedupStore {
    path: PathBuf,
    entries: HashMap<String, u64>,
}

impl DedupStore {
    pub fn load(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                entries: HashMap::new(),
            });
        }

        let content = fs::read_to_string(&path)?;
        let file: DedupFile = serde_json::from_str(&content)?;
        Ok(Self {
            path,
            entries: file.entries,
        })
    }

    pub fn should_process(
        &mut self,
        account_id: &str,
        event_id: Option<&str>,
        message_id: &str,
    ) -> Result<bool> {
        let now = unix_timestamp();
        self.entries
            .retain(|_, timestamp| now.saturating_sub(*timestamp) < DEDUP_TTL_SECS);

        let mut keys = vec![format!("msg:{account_id}:{message_id}")];
        if let Some(event_id) = event_id.filter(|value| !value.trim().is_empty()) {
            keys.push(format!("evt:{account_id}:{event_id}"));
        }

        if keys.iter().any(|key| self.entries.contains_key(key)) {
            return Ok(false);
        }

        while self.entries.len() + keys.len() > DEDUP_MAX_SIZE {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, timestamp)| *timestamp)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest_key);
            } else {
                break;
            }
        }

        for key in keys {
            self.entries.insert(key, now);
        }
        self.persist()?;
        Ok(true)
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = DedupFile {
            entries: self.entries.clone(),
        };
        fs::write(&self.path, serde_json::to_string_pretty(&file)?)?;
        Ok(())
    }
}

pub fn default_dedup_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?
        .join(".jellyfish")
        .join("feishu-dedup.json"))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn dedup_store_blocks_duplicate_message_id() {
        let path = temp_path("jellyfish-feishu-dedup-test.json");
        if Path::new(&path).exists() {
            let _ = fs::remove_file(&path);
        }
        let mut store = DedupStore::load(path.clone()).unwrap();

        assert!(store
            .should_process("main", Some("evt_1"), "msg_1")
            .unwrap());
        assert!(!store
            .should_process("main", Some("evt_2"), "msg_1")
            .unwrap());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn dedup_store_blocks_duplicate_event_id() {
        let path = temp_path("jellyfish-feishu-dedup-event-test.json");
        if Path::new(&path).exists() {
            let _ = fs::remove_file(&path);
        }
        let mut store = DedupStore::load(path.clone()).unwrap();

        assert!(store
            .should_process("main", Some("evt_1"), "msg_1")
            .unwrap());
        assert!(!store
            .should_process("main", Some("evt_1"), "msg_2")
            .unwrap());

        let _ = fs::remove_file(path);
    }
}
