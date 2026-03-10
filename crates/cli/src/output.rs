use std::path::Path;

use jellyfish_agent::AgentResponse;
use jellyfish_core::EventKind;

pub fn print_agent_response(
    response: &AgentResponse,
    memory_updates: &[String],
    session_path: &Path,
) {
    println!("Jellyfish:");
    println!("{}", response.message);

    let progress = response
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::System))
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>();
    if !progress.is_empty() {
        println!("\nProgress:");
        for item in progress {
            println!("- {}", item);
        }
    }

    let confirmations = response
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ConfirmationRequired))
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>();
    if !confirmations.is_empty() {
        println!("\nConfirmation:");
        for item in confirmations {
            println!("- {}", item);
        }
    }

    let completed_tools = response
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ToolCompleted))
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>();
    let failed_tools = response
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ToolFailed))
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>();

    if !completed_tools.is_empty() || !failed_tools.is_empty() || !memory_updates.is_empty() {
        println!("\nSummary:");
        if !completed_tools.is_empty() {
            println!("- completed tools: {}", completed_tools.len());
            for item in completed_tools {
                println!("- tool ok: {}", item);
            }
        }
        if !failed_tools.is_empty() {
            println!("- failed tools: {}", failed_tools.len());
            for item in failed_tools {
                println!("- tool failed: {}", item);
            }
        }
        for update in memory_updates {
            println!("- memory: {}", update);
        }
    }

    println!("\nSession:");
    println!("- saved to {}", session_path.display());
}
