use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::{Duration, timeout};
use tracing::info;

use rig::{
    agent::AgentBuilder,
    client::CompletionClient,
    completion::Prompt,
    providers::openai,
};

use jellyfish_core::{AgentEvent, AppConfig, AppError, AppResult, EventKind, Session};
use jellyfish_tools::{ApplyPatchTool, GlobTool, GrepTool, NoteTool, ReadTool, TodoTool, ToolRegistry};

use crate::{codex_auth, codex_cli, codex_runtime, prompt::PromptTemplate, traits::AgentRuntime};

const MAX_TOOL_TURNS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRequest {
    pub input: String,
    pub session: Option<Session>,
    pub retrieval_context: Vec<String>,
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
        jellyfish_core::ProviderKind::OpenAi => Ok(Box::new(RigAgentRuntime::new(config))),
        jellyfish_core::ProviderKind::Codex => Ok(Box::new(NativeCodexRuntime::new(config))),
        jellyfish_core::ProviderKind::CodexCli => Ok(Box::new(CodexCliRuntime::new(config))),
        jellyfish_core::ProviderKind::Mock => Ok(Box::new(MockAgentRuntime::new(config))),
        jellyfish_core::ProviderKind::Anthropic => Err(AppError::Config(
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
        tools.register(NoteTool::new(config.workspace_root.clone()));
        tools.register(TodoTool::new(config.workspace_root.clone()));
        if config.enable_repo_tools {
            tools.register(ReadTool::new(config.workspace_root.clone()));
            tools.register(GlobTool::new(config.workspace_root.clone()));
            tools.register(GrepTool::new(config.workspace_root.clone()));
            tools.register(ApplyPatchTool::new(config.workspace_root.clone()));
        }

        Self {
            config,
            prompt: PromptTemplate::personal_assistant(),
            tools,
        }
    }

    async fn prompt_model(&self, prompt: &str) -> AppResult<String> {
        let client = self.openai_compatible_client()?;
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

    fn openai_compatible_client(&self) -> AppResult<openai::Client> {
        if let Some(api_key) = codex_auth::load_bearer_token()? {
            let mut builder = openai::Client::builder().api_key(&api_key);

            if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
                if !base_url.trim().is_empty() {
                    builder = builder.base_url(&base_url);
                }
            }

            return builder
                .build()
                .map_err(|error| AppError::Config(format!("failed to build OpenAI-compatible client: {error}")));
        }

        let message = match self.config.provider {
            jellyfish_core::ProviderKind::Codex => {
                "codex profile selected, but Jellyfish could not find a usable bearer token in OPENAI_API_KEY or ~/.codex/auth.json. Sign in with Codex CLI first, or set OPENAI_API_KEY, or switch to mock.".to_string()
            }
            _ => "OPENAI_API_KEY is required for the current provider".to_string(),
        };

        Err(AppError::Config(message))
    }

    fn tool_instructions(&self) -> String {
        if self.tools.is_empty() {
            return "- none enabled".to_string();
        }

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

    fn memory_context(&self, session: Option<&Session>) -> String {
        let Some(session) = session else {
            return "No remembered user context yet.".to_string();
        };

        let display_name = session
            .profile
            .display_name
            .as_deref()
            .unwrap_or("unknown");
        let locale = session.profile.locale.as_deref().unwrap_or("unknown");
        let timezone = session.profile.timezone.as_deref().unwrap_or("unknown");
        let preferences = if session.profile.preferences.is_empty() {
            "none".to_string()
        } else {
            session
                .profile
                .preferences
                .iter()
                .map(|entry| format!("{}={}", entry.key, entry.value))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let memories = {
            let items = session.relevant_memories("recent user context preferences todos notes", 5);
            if items.is_empty() {
                "none".to_string()
            } else {
                items.join(" | ")
            }
        };

        let recent_messages = session
            .messages
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .join(" | ");

        format!(
            "User profile: display_name={display_name}, locale={locale}, timezone={timezone}. Preferences: {preferences}. Relevant memories: {memories}. Recent conversation: {recent_messages}."
        )
    }

    async fn call_tool(&self, tool_name: &str, input: Value) -> AppResult<jellyfish_tools::ToolOutput> {
        let output = timeout(
            Duration::from_secs(self.config.tool_timeout_secs),
            self.tools.call(tool_name, input),
        )
        .await
        .map_err(|_| AppError::Tool(format!("tool timed out: {tool_name}")))??;

        Ok(output.truncated(self.config.tool_output_max_chars))
    }

    fn build_step_prompt(&self, user_input: &str, transcript: &[String], session: Option<&Session>) -> String {
        let transcript = if transcript.is_empty() {
            "No tool interactions yet.".to_string()
        } else {
            transcript.join("\n\n")
        };
        let memory_context = self.memory_context(session);

        let tool_usage_guidance = if self.tools.is_empty() {
            "No tools are enabled for this run. Answer directly using conversation context and remembered user context."
                .to_string()
        } else {
            "If you need additional local context, you may return {\"kind\":\"tool\",\"tool_name\":\"...\",\"input\":{...}}."
                .to_string()
        };

        format!(
            concat!(
                "Decide the next step for this personal assistant request.\n",
                "Remembered user context:\n{memory_context}\n\n",
                "Safety policy:\n{file_edit_policy}\n\n",
                "Available tools:\n{tools}\n\n",
                "Tool guidance:\n{tool_usage_guidance}\n\n",
                "Conversation state:\n{transcript}\n\n",
                "User request:\n{user_input}\n\n",
                "Return JSON only.\n",
                "If you can answer, return:\n",
                "{{\"kind\":\"respond\",\"message\":\"final answer\"}}\n",
                "Do not include markdown fences or explanatory text outside JSON."
            ),
            memory_context = memory_context,
            file_edit_policy = if self.config.allow_file_edits {
                "File edits are allowed for this run when necessary."
            } else {
                "File edits are disabled unless the user explicitly enables them for this run."
            },
            tools = self.tool_instructions(),
            tool_usage_guidance = tool_usage_guidance,
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

    fn retrieval_context(&self, items: &[String]) -> String {
        if items.is_empty() {
            "No additional retrieval context found.".to_string()
        } else {
            items.join(" | ")
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
        events.push(AgentEvent {
            kind: EventKind::System,
            message: "Analyzing request and recalled context".to_string(),
        });
        if !request.retrieval_context.is_empty() {
            events.push(AgentEvent {
                kind: EventKind::System,
                message: format!(
                    "Retrieved {} relevant memory items",
                    request.retrieval_context.len()
                ),
            });
        }
        let mut transcript = Vec::new();
        let session = request.session.as_ref();

        for turn in 0..MAX_TOOL_TURNS {
            info!(turn, "running agent turn");
            let mut prompt = self.build_step_prompt(&request.input, &transcript, session);
            let retrieval_context = self.retrieval_context(&request.retrieval_context);
            prompt.push_str(&format!(
                "\n\nRetrieved context:\n{}\n",
                retrieval_context
            ));
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

                    if self.tools.is_empty() {
                        events.push(AgentEvent {
                            kind: EventKind::System,
                            message: "Model requested a tool, but no tools are enabled".to_string(),
                        });
                        return Ok(Self::fallback_response(raw, events));
                    }

                    if tool_name == "apply_patch" && !self.config.allow_file_edits {
                        let message =
                            "File edits require explicit confirmation. Re-run with --yes or set RIG_ALLOW_FILE_EDITS=true."
                                .to_string();
                        events.push(AgentEvent {
                            kind: EventKind::ConfirmationRequired,
                            message: message.clone(),
                        });
                        transcript.push(format!(
                            "Tool call on turn {} blocked:\nname={}\ninput={}\nreason={}",
                            turn + 1,
                            tool_name,
                            input,
                            message
                        ));
                        continue;
                    }

                    events.push(AgentEvent {
                        kind: EventKind::ToolRequested,
                        message: format!("{} {}", tool_name, input),
                    });
                    events.push(AgentEvent {
                        kind: EventKind::ToolStarted,
                        message: tool_name.clone(),
                    });

                    let output = match self.call_tool(&tool_name, input.clone()).await {
                        Ok(output) => {
                            events.push(AgentEvent {
                                kind: EventKind::ToolCompleted,
                                message: format!("{} -> {}", tool_name, output.content),
                            });
                            output
                        }
                        Err(error) => {
                            let message = error.to_string();
                            events.push(AgentEvent {
                                kind: EventKind::ToolFailed,
                                message: format!("{} -> {}", tool_name, message),
                            });
                            transcript.push(format!(
                                "Tool call on turn {} failed:\nname={}\ninput={}\nerror={}",
                                turn + 1,
                                tool_name,
                                input,
                                message
                            ));
                            continue;
                        }
                    };

                    transcript.push(format!(
                        "Tool call on turn {}:\nname={}\ninput={}\nresult={}",
                        turn + 1,
                        tool_name,
                        input,
                        output.content
                    ));
                    events.push(AgentEvent {
                        kind: EventKind::System,
                        message: "Incorporating tool results into the answer".to_string(),
                    });
                }
                _ => {
                    return Ok(Self::fallback_response(raw, events));
                }
            }
        }

        let prompt = format!(
            "The tool turn limit was reached. Answer the personal assistant request directly using the gathered context, remembered user context, and retrieved context.\n\nRemembered user context:\n{}\n\nRetrieved context:\n{}\n\nContext:\n{}\n\nUser request:\n{}",
            self.memory_context(session),
            self.retrieval_context(&request.retrieval_context),
            transcript.join("\n\n"),
            request.input
        );
        let message = self.prompt_model(&prompt).await?;
        events.push(AgentEvent {
            kind: EventKind::System,
            message: "Preparing final response".to_string(),
        });
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

pub struct NativeCodexRuntime {
    config: AppConfig,
    prompt: PromptTemplate,
    tools: ToolRegistry,
}

impl NativeCodexRuntime {
    pub fn new(config: AppConfig) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register(NoteTool::new(config.workspace_root.clone()));
        tools.register(TodoTool::new(config.workspace_root.clone()));
        if config.enable_repo_tools {
            tools.register(ReadTool::new(config.workspace_root.clone()));
            tools.register(GlobTool::new(config.workspace_root.clone()));
            tools.register(GrepTool::new(config.workspace_root.clone()));
            tools.register(ApplyPatchTool::new(config.workspace_root.clone()));
        }

        Self {
            config,
            prompt: PromptTemplate::personal_assistant(),
            tools,
        }
    }

    fn tool_instructions(&self) -> String {
        if self.tools.is_empty() {
            return "- none enabled".to_string();
        }

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

    fn memory_context(&self, session: Option<&Session>) -> String {
        let Some(session) = session else {
            return "No remembered user context yet.".to_string();
        };

        let display_name = session.profile.display_name.as_deref().unwrap_or("unknown");
        let locale = session.profile.locale.as_deref().unwrap_or("unknown");
        let timezone = session.profile.timezone.as_deref().unwrap_or("unknown");
        let preferences = if session.profile.preferences.is_empty() {
            "none".to_string()
        } else {
            session
                .profile
                .preferences
                .iter()
                .map(|entry| format!("{}={}", entry.key, entry.value))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let memories = {
            let items = session.relevant_memories("recent user context preferences todos notes", 5);
            if items.is_empty() {
                "none".to_string()
            } else {
                items.join(" | ")
            }
        };
        let recent_messages = session
            .messages
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .join(" | ");

        format!(
            "User profile: display_name={display_name}, locale={locale}, timezone={timezone}. Preferences: {preferences}. Relevant memories: {memories}. Recent conversation: {recent_messages}."
        )
    }

    async fn call_tool(&self, tool_name: &str, input: Value) -> AppResult<jellyfish_tools::ToolOutput> {
        let output = timeout(
            Duration::from_secs(self.config.tool_timeout_secs),
            self.tools.call(tool_name, input),
        )
        .await
        .map_err(|_| AppError::Tool(format!("tool timed out: {tool_name}")))??;

        Ok(output.truncated(self.config.tool_output_max_chars))
    }

    fn build_step_prompt(&self, user_input: &str, transcript: &[String], session: Option<&Session>) -> String {
        let transcript = if transcript.is_empty() {
            "No tool interactions yet.".to_string()
        } else {
            transcript.join("\n\n")
        };
        let memory_context = self.memory_context(session);
        let tool_usage_guidance = if self.tools.is_empty() {
            "No tools are enabled for this run. Answer directly using conversation context and remembered user context."
                .to_string()
        } else {
            "If you need additional local context, you may return {\"kind\":\"tool\",\"tool_name\":\"...\",\"input\":{...}}."
                .to_string()
        };

        format!(
            concat!(
                "Decide the next step for this personal assistant request.\n",
                "Remembered user context:\n{memory_context}\n\n",
                "Safety policy:\n{file_edit_policy}\n\n",
                "Available tools:\n{tools}\n\n",
                "Tool guidance:\n{tool_usage_guidance}\n\n",
                "Conversation state:\n{transcript}\n\n",
                "User request:\n{user_input}\n\n",
                "Return JSON only.\n",
                "If you can answer, return:\n",
                "{{\"kind\":\"respond\",\"message\":\"final answer\"}}\n",
                "Do not include markdown fences or explanatory text outside JSON."
            ),
            memory_context = memory_context,
            file_edit_policy = if self.config.allow_file_edits {
                "File edits are allowed for this run when necessary."
            } else {
                "File edits are disabled unless the user explicitly enables them for this run."
            },
            tools = self.tool_instructions(),
            tool_usage_guidance = tool_usage_guidance,
            transcript = transcript,
            user_input = user_input,
        )
    }

    fn retrieval_context(&self, items: &[String]) -> String {
        if items.is_empty() {
            "No additional retrieval context found.".to_string()
        } else {
            items.join(" | ")
        }
    }

    async fn prompt_model(
        &self,
        credentials: &codex_auth::CodexCredentials,
        prompt: &str,
        retrieval_context: &[String],
    ) -> AppResult<codex_runtime::CodexRunResult> {
        codex_runtime::run_codex_request(
            credentials,
            &self.config.model,
            &self.prompt.system,
            prompt,
            retrieval_context,
            &self.config.codex_transport,
        )
        .await
    }
}

#[async_trait]
impl AgentRuntime for NativeCodexRuntime {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse> {
        let credentials = codex_auth::load_codex_credentials()?.ok_or_else(|| {
            AppError::Config(
                "codex auth cache was not found or did not contain usable OAuth credentials"
                    .to_string(),
            )
        })?;

        let mut events = vec![AgentEvent {
            kind: EventKind::UserMessage,
            message: request.input.clone(),
        }];
        events.push(AgentEvent {
            kind: EventKind::System,
            message: "Analyzing request and recalled context".to_string(),
        });
        events.push(AgentEvent {
            kind: EventKind::System,
            message: format!("Provider profile: {}", self.config.provider.as_str()),
        });
        events.push(AgentEvent {
            kind: EventKind::System,
            message: format!("Codex native model: {}", self.config.model),
        });
        if !request.retrieval_context.is_empty() {
            events.push(AgentEvent {
                kind: EventKind::System,
                message: format!(
                    "Retrieved {} relevant memory items",
                    request.retrieval_context.len()
                ),
            });
        }

        let mut transcript = Vec::new();
        let session = request.session.as_ref();

        for turn in 0..MAX_TOOL_TURNS {
            tracing::debug!(turn, "running native codex agent turn");
            let mut prompt = self.build_step_prompt(&request.input, &transcript, session);
            let retrieval_context = self.retrieval_context(&request.retrieval_context);
            prompt.push_str(&format!("\n\nRetrieved context:\n{}\n", retrieval_context));
            let result = self
                .prompt_model(&credentials, &prompt, &request.retrieval_context)
                .await?;
            let raw = result.message;
            events.push(AgentEvent {
                kind: EventKind::System,
                message: format!("Codex transport used: {}", result.transport.as_str()),
            });
            if result.refreshed {
                events.push(AgentEvent {
                    kind: EventKind::System,
                    message: "Codex credentials refreshed during request".to_string(),
                });
            }

            let Some(step) = RigAgentRuntime::parse_step(&raw) else {
                events.push(AgentEvent {
                    kind: EventKind::AgentMessage,
                    message: "Model returned non-JSON fallback response".to_string(),
                });
                return Ok(RigAgentRuntime::fallback_response(raw, events));
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

                    if self.tools.is_empty() {
                        events.push(AgentEvent {
                            kind: EventKind::System,
                            message: "Model requested a tool, but no tools are enabled".to_string(),
                        });
                        return Ok(RigAgentRuntime::fallback_response(raw, events));
                    }

                    if tool_name == "apply_patch" && !self.config.allow_file_edits {
                        let message = "File edits require explicit confirmation. Re-run with --yes or set RIG_ALLOW_FILE_EDITS=true.".to_string();
                        events.push(AgentEvent {
                            kind: EventKind::ConfirmationRequired,
                            message: message.clone(),
                        });
                        transcript.push(format!(
                            "Tool call on turn {} blocked:\nname={}\ninput={}\nreason={}",
                            turn + 1,
                            tool_name,
                            input,
                            message
                        ));
                        continue;
                    }

                    events.push(AgentEvent {
                        kind: EventKind::ToolRequested,
                        message: format!("{} {}", tool_name, input),
                    });
                    events.push(AgentEvent {
                        kind: EventKind::ToolStarted,
                        message: tool_name.clone(),
                    });

                    let output = match self.call_tool(&tool_name, input.clone()).await {
                        Ok(output) => {
                            events.push(AgentEvent {
                                kind: EventKind::ToolCompleted,
                                message: format!("{} -> {}", tool_name, output.content),
                            });
                            output
                        }
                        Err(error) => {
                            let message = error.to_string();
                            events.push(AgentEvent {
                                kind: EventKind::ToolFailed,
                                message: format!("{} -> {}", tool_name, message),
                            });
                            transcript.push(format!(
                                "Tool call on turn {} failed:\nname={}\ninput={}\nerror={}",
                                turn + 1,
                                tool_name,
                                input,
                                message
                            ));
                            continue;
                        }
                    };

                    transcript.push(format!(
                        "Tool call on turn {}:\nname={}\ninput={}\nresult={}",
                        turn + 1,
                        tool_name,
                        input,
                        output.content
                    ));
                    events.push(AgentEvent {
                        kind: EventKind::System,
                        message: "Incorporating tool results into the answer".to_string(),
                    });
                }
                _ => return Ok(RigAgentRuntime::fallback_response(raw, events)),
            }
        }

        let prompt = format!(
            "The tool turn limit was reached. Answer the personal assistant request directly using the gathered context, remembered user context, and retrieved context.\n\nRemembered user context:\n{}\n\nRetrieved context:\n{}\n\nContext:\n{}\n\nUser request:\n{}",
            self.memory_context(session),
            self.retrieval_context(&request.retrieval_context),
            transcript.join("\n\n"),
            request.input
        );
        let result = self
            .prompt_model(&credentials, &prompt, &request.retrieval_context)
            .await?;
        let message = result.message;
        events.push(AgentEvent {
            kind: EventKind::System,
            message: format!("Codex transport used: {}", result.transport.as_str()),
        });
        if result.refreshed {
            events.push(AgentEvent {
                kind: EventKind::System,
                message: "Codex credentials refreshed during request".to_string(),
            });
        }
        events.push(AgentEvent {
            kind: EventKind::System,
            message: "Preparing final response".to_string(),
        });
        events.push(AgentEvent {
            kind: EventKind::AgentMessage,
            message: message.clone(),
        });

        Ok(AgentResponse { message, events })
    }
}

pub struct CodexCliRuntime {
    config: AppConfig,
}

impl CodexCliRuntime {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl AgentRuntime for CodexCliRuntime {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse> {
        if !codex_cli::codex_cli_available() {
            return Err(AppError::Config(
                "codex CLI is not installed or not available in PATH".to_string(),
            ));
        }
        if !codex_cli::codex_auth_cache_exists() {
            return Err(AppError::Config(
                "codex CLI auth cache was not found at ~/.codex/auth.json".to_string(),
            ));
        }

        let mut prompt = request.input.clone();
        if !request.retrieval_context.is_empty() {
            prompt.push_str("\n\nRelevant context:\n");
            prompt.push_str(&request.retrieval_context.join("\n"));
        }

        let message = codex_cli::run_codex_exec(
            &self.config.model,
            &prompt,
            &self.config.workspace_root,
        )?;

        Ok(AgentResponse {
            message,
            events: vec![
                AgentEvent {
                    kind: EventKind::System,
                    message: "Analyzing request and recalled context".to_string(),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Provider profile: {}", self.config.provider.as_str()),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Codex CLI model: {}", self.config.model),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: if request.retrieval_context.is_empty() {
                        "Retrieved 0 relevant memory items".to_string()
                    } else {
                        format!(
                            "Retrieved {} relevant memory items",
                            request.retrieval_context.len()
                        )
                    },
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: "Preparing final response".to_string(),
                },
            ],
        })
    }
}

impl MockAgentRuntime {
    pub fn new(config: AppConfig) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register(NoteTool::new(config.workspace_root.clone()));
        tools.register(TodoTool::new(config.workspace_root.clone()));
        if config.enable_repo_tools {
            tools.register(ReadTool::new(config.workspace_root.clone()));
            tools.register(GlobTool::new(config.workspace_root.clone()));
            tools.register(GrepTool::new(config.workspace_root.clone()));
            tools.register(ApplyPatchTool::new(config.workspace_root.clone()));
        }

        Self { config, tools }
    }
}

#[async_trait]
impl AgentRuntime for MockAgentRuntime {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse> {
        let tool_names = if self.tools.is_empty() {
            "none".to_string()
        } else {
            self.tools.names().join(", ")
        };

        Ok(AgentResponse {
            message: format!(
                "Mock personal assistant runtime active for model {}. Input: {}",
                self.config.model, request.input
            ),
            events: vec![
                AgentEvent {
                    kind: EventKind::System,
                    message: "Analyzing request and recalled context".to_string(),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Workspace root: {}", self.config.workspace_root.display()),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Provider profile: {}", self.config.provider.as_str()),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!("Registered tools: {}", tool_names),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: if self.config.allow_file_edits {
                        "Dangerous file edits are allowed for this run".to_string()
                    } else {
                        "Dangerous file edits require explicit confirmation".to_string()
                    },
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: match request.session.as_ref() {
                        Some(session) => format!(
                            "Memory available: {} profile fields, {} memory entries",
                            usize::from(session.profile.display_name.is_some())
                                + usize::from(session.profile.locale.is_some())
                                + usize::from(session.profile.timezone.is_some())
                                + session.profile.preferences.len(),
                            session.memories.len()
                        ),
                        None => "Memory available: none".to_string(),
                    },
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: format!(
                        "Retrieved {} relevant memory items",
                        request.retrieval_context.len()
                    ),
                },
                AgentEvent {
                    kind: EventKind::System,
                    message: "Preparing final response".to_string(),
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
