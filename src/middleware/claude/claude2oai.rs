use async_stream::try_stream;
use axum::response::sse::Event;
use futures::{Stream, StreamExt};
use serde::Serialize;
use serde_json::Value;

use crate::types::claude::{ContentBlockDelta, CreateMessageResponse, StreamEvent};

/// Represents the data structure for streaming events in OpenAI API format
/// Contains a choices array with deltas of content
#[derive(Debug, Serialize)]
struct StreamEventData {
    choices: Vec<StreamEventDelta>,
}

impl StreamEventData {
    fn new(content: EventContent) -> Self {
        Self {
            choices: vec![StreamEventDelta {
                delta: content,
                finish_reason: None,
            }],
        }
    }

    fn finished(reason: &'static str) -> Self {
        Self {
            choices: vec![StreamEventDelta {
                delta: EventContent::Content {
                    content: String::new(),
                },
                finish_reason: Some(reason),
            }],
        }
    }
}

/// Represents a delta update in a streaming response
/// Contains the content change for the current chunk
#[derive(Debug, Serialize)]
struct StreamEventDelta {
    delta: EventContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<&'static str>,
}

/// Content of an event, either regular content or reasoning (thinking mode)
/// Uses untagged enum to handle different response formats
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum EventContent {
    Content { content: String },
    Reasoning { reasoning_content: String },
}

/// Creates an SSE event with the given content in OpenAI format
///
/// # Arguments
/// * `content` - The event content to include
///
/// # Returns
/// A formatted SSE Event ready to be sent to the client
pub fn build_event(content: EventContent) -> Event {
    let event = Event::default();
    let data = StreamEventData::new(content);
    event.json_data(data).unwrap()
}

/// Transforms a Claude.ai event stream into an OpenAI-compatible event stream
///
/// Extracts content from Claude events and reformats them to match OpenAI's streaming format.
/// This function processes each event in the stream, identifying the delta content type
/// (text or thinking), and converting it to the appropriate OpenAI-compatible event format.
///
/// # Arguments
/// * `s` - The input stream of Claude.ai events
///
/// # Returns
/// A stream of OpenAI-compatible SSE events
///
/// # Type Parameters
/// * `I` - The input stream type
/// * `E` - The error type for the stream
pub fn transform_stream<I, E>(s: I) -> impl Stream<Item = Result<Event, E>>
where
    I: Stream<Item = Result<eventsource_stream::Event, E>>,
{
    try_stream! {
        futures::pin_mut!(s);
        let mut finish_reason = "stop";
        let mut sent_terminal = false;

        while let Some(event) = s.next().await {
            let event = event?;
            let Ok(parsed) = serde_json::from_str::<StreamEvent>(&event.data) else {
                continue;
            };
            match parsed {
                StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                    ContentBlockDelta::TextDelta { text } => {
                        yield build_event(EventContent::Content { content: text });
                    }
                    ContentBlockDelta::ThinkingDelta { thinking } => {
                        yield build_event(EventContent::Reasoning {
                            reasoning_content: thinking,
                        });
                    }
                    _ => {}
                },
                StreamEvent::MessageDelta { delta, .. } => {
                    finish_reason = match delta.stop_reason {
                        Some(crate::types::claude::StopReason::MaxTokens)
                        | Some(crate::types::claude::StopReason::ModelContextWindowExceeded) => "length",
                        Some(crate::types::claude::StopReason::ToolUse) => "tool_calls",
                        Some(crate::types::claude::StopReason::Refusal) => "content_filter",
                        _ => "stop",
                    };
                }
                StreamEvent::MessageStop => {
                    yield Event::default().json_data(StreamEventData::finished(finish_reason)).unwrap();
                    yield Event::default().data("[DONE]");
                    sent_terminal = true;
                }
                _ => {}
            }
        }

        // Some Claude Web streams close without a native message_stop event.
        // Still terminate the OpenAI-compatible stream explicitly on clean EOF.
        if !sent_terminal {
            yield Event::default().json_data(StreamEventData::finished(finish_reason)).unwrap();
            yield Event::default().data("[DONE]");
        }
    }
}

pub fn transforms_json(input: CreateMessageResponse) -> Value {
    let content = input
        .content
        .iter()
        .filter_map(|block| match block {
            crate::types::claude::ContentBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<String>();

    let usage = input.usage.as_ref().map(|u| {
        serde_json::json!({
            "prompt_tokens": u.input_tokens,
            "completion_tokens": u.output_tokens,
            "total_tokens": u.input_tokens + u.output_tokens
        })
    });

    let finish_reason = match input.stop_reason {
        Some(crate::types::claude::StopReason::EndTurn) => "stop",
        Some(crate::types::claude::StopReason::MaxTokens) => "length",
        Some(crate::types::claude::StopReason::StopSequence) => "stop",
        Some(crate::types::claude::StopReason::ToolUse) => "tool_calls",
        Some(crate::types::claude::StopReason::PauseTurn) => "stop",
        Some(crate::types::claude::StopReason::Refusal) => "content_filter",
        Some(crate::types::claude::StopReason::ModelContextWindowExceeded) => "length",
        None => "stop",
    };

    serde_json::json!({
        "id": input.id,
        "object": "chat.completion",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "model": input.model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": finish_reason
        }],
        "usage": usage
    })
}
