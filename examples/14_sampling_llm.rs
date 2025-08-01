//! Example of implementing a sampling/LLM server.

use async_trait::async_trait;
use pmcp::{
    types::{protocol::{Content, CreateMessageParams, CreateMessageResult, TokenUsage}, capabilities::ServerCapabilities},
    SamplingHandler, Server,
};
use tracing::info;

struct MockLLM {
    model_name: String,
}

#[async_trait]
impl SamplingHandler for MockLLM {
    async fn create_message(
        &self,
        params: CreateMessageParams,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<CreateMessageResult> {
        info!(
            "Received sampling request with {} messages",
            params.messages.len()
        );

        // In a real implementation, this would call an actual LLM
        let response_text = format!(
            "This is a mock response to: {}",
            params
                .messages
                .last()
                .map(|m| match &m.content {
                    Content::Text { text } => text.as_str(),
                    Content::Image { .. } => "[image]",
                    Content::Resource { .. } => "[resource]",
                })
                .unwrap_or("empty")
        );

        Ok(CreateMessageResult {
            content: Content::Text {
                text: response_text,
            },
            model: self.model_name.clone(),
            usage: Some(TokenUsage {
                input_tokens: params.messages.len() as u32 * 10,
                output_tokens: 20,
                total_tokens: params.messages.len() as u32 * 10 + 20,
            }),
            stop_reason: Some("end_of_text".to_string()),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Creating LLM server");

    let server = Server::builder()
        .name("mock-llm-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            sampling: Some(Default::default()),
            ..Default::default()
        })
        .sampling(MockLLM {
            model_name: "mock-gpt-4".to_string(),
        })
        .build()?;

    info!("Starting server on stdio");
    server.run_stdio().await?;

    Ok(())
}
