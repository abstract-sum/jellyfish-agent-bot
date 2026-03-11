use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use jellyfish_agent::{AgentRequest, build_runtime};
use jellyfish_core::{AppConfig, MessageRole, Session};
use jellyfish_schema::{InboundMessage, OutboundMessage};

use crate::routing::session_locator_for_message;
use crate::session_key::build_session_key;

#[async_trait]
pub trait GatewayService: Send + Sync {
    async fn handle_inbound(&self, msg: InboundMessage) -> Result<OutboundMessage>;
}

pub struct JellyfishGateway {
    config: AppConfig,
}

impl JellyfishGateway {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl GatewayService for JellyfishGateway {
    async fn handle_inbound(&self, msg: InboundMessage) -> Result<OutboundMessage> {
        let runtime = build_runtime(self.config.clone())?;
        let locator = session_locator_for_message(&msg);
        let session_key = build_session_key(&locator);

        let mut session = load_session(&self.config.workspace_root, &session_key)?;
        session.push_message(MessageRole::User, msg.text.clone());

        let retrieval_context = session.relevant_memories(&msg.text, 6);
        let response = runtime
            .run(AgentRequest {
                input: msg.text.clone(),
                session: Some(session.clone()),
                retrieval_context,
            })
            .await?;

        session.push_message(MessageRole::Assistant, response.message.clone());
        for event in response.events {
            session.push_event(event);
        }
        save_session(&self.config.workspace_root, &session_key, &session)?;

        Ok(OutboundMessage {
            channel: msg.channel,
            account_id: msg.account_id,
            peer: msg.peer,
            reply_to_message_id: Some(msg.message_id),
            text: response.message,
        })
    }
}

fn session_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".jellyfish").join("channels")
}

fn session_file_path(workspace_root: &Path, session_key: &str) -> PathBuf {
    let sanitized = session_key.replace(':', "__");
    session_dir(workspace_root).join(format!("{sanitized}.json"))
}

fn load_session(workspace_root: &Path, session_key: &str) -> Result<Session> {
    let path = session_file_path(workspace_root, session_key);
    if !path.exists() {
        return Ok(Session::new());
    }

    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_session(workspace_root: &Path, session_key: &str, session: &Session) -> Result<()> {
    let dir = session_dir(workspace_root);
    fs::create_dir_all(&dir)?;
    let path = session_file_path(workspace_root, session_key);
    fs::write(path, serde_json::to_string_pretty(session)?)?;
    Ok(())
}
