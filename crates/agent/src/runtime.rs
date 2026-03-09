use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

use rig::{
    agent::AgentBuilder,
    client::{CompletionClient, ProviderClient},
    completion::Prompt,
    providers::openai,
};

use openclaw_core::{AgentEvent, AppConfig, AppError, AppResult, EventKind};
use openclaw_tools::{GlobTool, GrepTool, ReadTool, ToolRegistry};

use crate::{prompt::PromptTemplate, traits::AgentRuntime};

const MAX_TOOL_TURNS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRequest {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentResponse {
    pub message: String,
    pub events: Vec<AgentEvent>,
}

#[derive(Debug, Deserialize)]
struct AgentStep {
    kind: String,
    message: Option<String>,
    tool_name: Option<String>,
    input: Option<Value>,
}

pub fn build_runtime(config: AppConfig) -> AppResult<Box<dyn AgentRuntime>> {
    match config.provider {
        openclaw_core::ProviderKind::OpenAi => Ok(Box::new(RigAgentRuntime::new(config))),
        openclaw_core::ProviderKind::Mock => Ok(Box::new(MockAgentRuntime::new(config))),
        openclaw_core::ProviderKind::Anthropic => Err(AppError::Config(
            "anthropic provider is not wired yet in Phase 1".to_string(),
        )),
    }
}

pub struct RigAgentRuntime {
    config: AppConfig,
    prompt: PromptTemplate,
    tools: ToolRegistry,
}

impl RigAgentRuntime {
    pub fn new(config: AppConfig) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register(ReadTool::new(config.workspace_root.clone()));
        tools.register(GlobTool::new(config.workspace_root.clone()));
        tools.register(GrepTool::new(config.workspace_root.clone()));

        Self {
            config,
            prompt: PromptTemplate::coding_assistant(),
            tools,
        }
    }

    async fn prompt_model(&self, prompt: &str) -> AppResult<String> {
        let client = openai::Client::from_env();
        let agent = AgentBuilder::new(client.completion_model(&self.config.model))
            .preamble(&self.prompt.system)
            .temperature(0.1)
            .max_tokens(1200)
            .build();

        let response: String = agent
            .prompt(prompt)
            .await
            .map_err(|error| AppError::Runtime(error.to_string()))?;

        Ok(response)
    }

    fn tool_instructions(&self) -> String {
        self.tools
            .definitions()
            .into_iter()
            .map(|definition| {
                format!(
                    "- {}: {} | schema={} ",
                    definition.name, definition.description, definition.input_schema
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn build_step_prompt(&self, user_input: &str, transcript: &[String]) -> String {
        let transcript = if transcript.is_empty() {
            "No tool interactions yet.".to_string()
        } else {
            transcript.join("\n\n")
        };

        format!(
            concat!(
                "Decide the next step for this coding request.\n",
                "Available tools:\n{tools}\n\n",
                "Conversation state:\n{transcript}\n\n",
                "User request:\n{user_input}\n\n",
                "Return JSON only.\n",
                "If you need repository context, return:\n",
                "{{\"kind\":\"tool\",\"tool_name\":\"read|glob|grep\",\"input\":{{...}}}}\n",
                "If you can answer, return:\n",
                "{{\"kind\":\"respond\",\"message\":\"final answer\"}}\n",
                "Do not include markdown fences or explanatory text outside JSON."
            ),
            tools = self.tool_instructions(),
            transcript = transcript,
            user_input = user_input,
        )
    }

    fn parse_step(raw: &str) -> Option<AgentStep> {
        let candidate = raw.trim();
        if let Ok(step) = serde_json::from_str::<AgentStep>(candidate) {
            return Some(step);
        }

        let start = candidate.find('{')?;
        let end = candidate.rfind('}')?;
        serde_json::from_str::<AgentStep>(&candidate[start..=end]).ok()
    }

    fn fallback_response(raw: String, events: Vec<AgentEvent>) -> AgentResponse {
        AgentResponse {
            message: raw,
            events,
        }
    }
}

#[async_trait]
impl AgentRuntime for RigAgentRuntime {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse> {
        let mut events = vec![AgentEvent {
            kind: EventKind::UserMessage,
            message: request.input.clone(),
        }];
        let mut transcript = Vec::new();

        for turn in 0..MAX_TOOL_TURNS {
            info!(turn, "running agent turn");
            let prompt = self.build_step_prompt(&request.input, &transcript);
            let raw = self.prompt_model(&prompt).await?;

            let Some(step) = Self::parse_step(&raw) else {
                events.push(AgentEvent {
                    kind: EventKind::AgentMessage,
                    message: "Model returned non-JSON fallback response".to_string(),
                });
                return Ok(Self::fallback_response(raw, events));
            };

            match step.kind.as_str() {
                "respond" => {
                    let message = step.message.unwrap_or_else(|| raw.clone());
                    events.push(AgentEvent {
                        kind: EventKind::AgentMessage,
                        message: message.clone(),
                    });

                    return Ok(AgentResponse { message, events });
                }
                "tool" => {
                    let tool_name = step.tool_name.unwrap_or_default();
                    let input = step.input.unwrap_or_else(|| json!({}));

                    events.push(AgentEvent {
                        kind: EventKind::ToolCall,
                        message: format!("{} {}", tool_name, input),
                    });

                    let output = self.tools.call(&tool_name, input.clone()).await?;
                    events.push(AgentEvent {
                        kind: EventKind::ToolResult,
                        message: format!("{} -> {}", tool_name, output.content),
                    });

                    transcript.push(format!(
                        "Tool call on turn {}:\nname={}\ninput={}\nresult={}",
                        turn + 1,
                        tool_name,
                        input,
                        output.content
                    ));
                }
                _ => {
                    return Ok(Self::fallback_response(raw, events));
                }
            }
        }

        let prompt = format!(
            "The tool turn limit was reached. Based on this gathered context, answer the user directly.\n\nContext:\n{}\n\nUser request:\n{}",
            transcript.join("\n\n"),
            request.input
        );
        let message = self.prompt_model(&prompt).await?;
        events.push(AgentEvent {
            kind: EventKind::AgentMessage,
            message: message.clone(),
        });

        Ok(AgentResponse { message, events })
    }
}

pub struct MockAgentRuntime {
    config: AppConfig,
    tools: ToolRegistry,
}

impl MockAgentRuntime {
    pub fn new(config: AppConfig) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register(ReadTool::new(config.workspace_root.clone()));
        tools.register(GlobTool::new(config.workspace_root.clone()));
        tools.register(GrepTool::new(config.workspace_root.clone()));

        Self { config, tools }
    }
}

#[async_trait]
impl AgentRuntime for MockAgentRuntime {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse> {
        let tool_names = self.tools.names().join(", ");

        Ok(AgentResponse {
            message: format!(
                "Mock runtime active for model {}. Input: {}",
                self.config.model, request.input
            ),
            events: vec![
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Workspace root: {}", self.config.workspace_root.display()),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Registered tools: {}", tool_names),
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_step() {
        let step = RigAgentRuntime::parse_step(
            r#"{"kind":"tool","tool_name":"read","input":{"path":"README.md"}}"#,
        )
        .unwrap();

        assert_eq!(step.kind, "tool");
        assert_eq!(step.tool_name.as_deref(), Some("read"));
    }

    #[test]
    fn parses_json_wrapped_in_text() {
        let step = RigAgentRuntime::parse_step(
            "Here is the next action: {\"kind\":\"respond\",\"message\":\"done\"}",
        )
        .unwrap();

        assert_eq!(step.kind, "respond");
        assert_eq!(step.message.as_deref(), Some("done"));
    }
}
