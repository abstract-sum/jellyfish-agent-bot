use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use jellyfish_core::Session;

const SESSION_DIR: &str = ".jellyfish";
const SESSION_FILE: &str = "session.json";

pub fn load_or_create(workspace_root: &Path) -> Result<Session> {
    let path = session_file_path(workspace_root);
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let session = serde_json::from_str(&content)?;
        Ok(session)
    } else {
        Ok(Session::new())
    }
}

pub fn save(workspace_root: &Path, session: &Session) -> Result<PathBuf> {
    let dir = workspace_root.join(SESSION_DIR);
    fs::create_dir_all(&dir)?;

    let path = dir.join(SESSION_FILE);
    let content = serde_json::to_string_pretty(session)?;
    fs::write(&path, content)?;

    Ok(path)
}

pub fn session_file_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(SESSION_DIR).join(SESSION_FILE)
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
        let root = std::env::temp_dir().join(format!("jellyfish-cli-session-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn saves_and_loads_session() {
        let workspace = temp_workspace();
        let mut session = Session::new();
        session.set_display_name("User");

        let path = save(&workspace, &session).unwrap();
        assert!(path.exists());

        let loaded = load_or_create(&workspace).unwrap();
        assert_eq!(loaded.profile.display_name.as_deref(), Some("User"));

        fs::remove_dir_all(workspace).unwrap();
    }
}
