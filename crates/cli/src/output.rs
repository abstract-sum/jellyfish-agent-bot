use jellyfish_agent::AgentResponse;

pub fn print_agent_response(response: &AgentResponse) {
    println!("{}", response.message);

    for event in &response.events {
        println!("- [{:?}] {}", event.kind, event.message);
    }
}
