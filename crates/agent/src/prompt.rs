#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    pub system: String,
}

impl PromptTemplate {
    pub fn coding_assistant() -> Self {
        Self {
            system: "You are OpenClaw, a reliable coding assistant built on Rig.".to_string(),
        }
    }
}
