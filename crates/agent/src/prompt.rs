#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    pub system: String,
}

impl PromptTemplate {
    pub fn coding_assistant() -> Self {
        Self {
            system: [
                "You are OpenClaw, a reliable coding assistant built on Rig.",
                "You work inside a local source repository.",
                "Use tools when repository context is needed before answering.",
                "Keep answers concise and grounded in the tool results you see.",
                "When asked to choose the next step, respond with valid JSON only.",
            ]
            .join(" "),
        }
    }
}
