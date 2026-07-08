use std::{collections::VecDeque, sync::LazyLock};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::error;

use crate::{
    config::{CLEWDR_CONFIG, LOG_DIR},
    types::claude::{CreateMessageParams, MessageContent, Tool},
};

const MAX_LOGS: usize = 500;
const REQUEST_LOG_FILE: &str = "api_request_logs.json";

static REQUEST_LOGS: LazyLock<Mutex<RequestLogStore>> =
    LazyLock::new(|| Mutex::new(RequestLogStore::default()));

#[derive(Default)]
struct RequestLogStore {
    loaded: bool,
    next_id: u64,
    logs: VecDeque<ApiRequestLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiRequestLog {
    pub id: u64,
    pub timestamp_ms: i64,
    pub duration_ms: Option<i64>,
    pub provider: String,
    pub api_format: String,
    pub model: String,
    pub stream: bool,
    pub message_count: usize,
    pub tool_count: usize,
    pub cache_control_breakpoints: usize,
    pub estimated_context_tokens: u32,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub status: RequestLogStatus,
    pub error_code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RequestLogStatus {
    #[default]
    Pending,
    Success,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct UsageSnapshot {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

pub async fn record_start(
    provider: impl Into<String>,
    api_format: impl Into<String>,
    params: &CreateMessageParams,
) -> u64 {
    let mut store = REQUEST_LOGS.lock().await;
    ensure_loaded(&mut store).await;
    store.next_id = store.next_id.saturating_add(1);
    let id = store.next_id;
    let log = ApiRequestLog {
        id,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        provider: provider.into(),
        api_format: api_format.into(),
        model: params.model.clone(),
        stream: params.stream.unwrap_or(false),
        message_count: params.messages.len(),
        tool_count: params.tools.as_ref().map(|tools| tools.len()).unwrap_or(0),
        cache_control_breakpoints: count_cache_control_breakpoints(params),
        estimated_context_tokens: params.count_tokens(),
        status: RequestLogStatus::Pending,
        ..Default::default()
    };
    store.logs.push_front(log);
    while store.logs.len() > MAX_LOGS {
        store.logs.pop_back();
    }
    persist(&store).await;
    id
}

pub async fn record_success(id: u64, usage: UsageSnapshot) {
    update(id, |log| {
        log.duration_ms = Some(
            chrono::Utc::now()
                .timestamp_millis()
                .saturating_sub(log.timestamp_ms),
        );
        log.input_tokens = usage.input_tokens;
        log.output_tokens = usage.output_tokens;
        log.cache_creation_input_tokens = usage.cache_creation_input_tokens;
        log.cache_read_input_tokens = usage.cache_read_input_tokens;
        log.status = RequestLogStatus::Success;
        log.error = None;
    })
    .await;
}

pub async fn record_error(id: u64, error: impl Into<String>) {
    let error = error.into();
    let error_code = extract_error_code(&error);
    update(id, |log| {
        log.duration_ms = Some(
            chrono::Utc::now()
                .timestamp_millis()
                .saturating_sub(log.timestamp_ms),
        );
        log.status = RequestLogStatus::Error;
        log.error_code = error_code;
        log.error = Some(error);
    })
    .await;
}

pub async fn list(limit: Option<usize>) -> Vec<ApiRequestLog> {
    let mut store = REQUEST_LOGS.lock().await;
    ensure_loaded(&mut store).await;
    let limit = limit.unwrap_or(100).min(MAX_LOGS);
    store.logs.iter().take(limit).cloned().collect()
}

pub async fn clear() {
    let mut store = REQUEST_LOGS.lock().await;
    ensure_loaded(&mut store).await;
    store.logs.clear();
    store.next_id = 0;
    persist(&store).await;
}

async fn update(id: u64, f: impl FnOnce(&mut ApiRequestLog)) {
    let mut store = REQUEST_LOGS.lock().await;
    ensure_loaded(&mut store).await;
    if let Some(log) = store.logs.iter_mut().find(|log| log.id == id) {
        f(log);
        persist(&store).await;
    }
}

async fn ensure_loaded(store: &mut RequestLogStore) {
    if store.loaded {
        return;
    }
    store.loaded = true;

    if CLEWDR_CONFIG.load().no_fs {
        return;
    }

    let path = LOG_DIR.join(REQUEST_LOG_FILE);
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return;
    };
    let Ok(mut logs) = serde_json::from_slice::<Vec<ApiRequestLog>>(&bytes) else {
        error!("Failed to parse API request log file {}", path.display());
        return;
    };

    logs.sort_by(|a, b| {
        b.timestamp_ms
            .cmp(&a.timestamp_ms)
            .then_with(|| b.id.cmp(&a.id))
    });
    logs.truncate(MAX_LOGS);
    store.next_id = logs.iter().map(|log| log.id).max().unwrap_or_default();
    store.logs = logs.into();
}

async fn persist(store: &RequestLogStore) {
    if CLEWDR_CONFIG.load().no_fs {
        return;
    }

    let path = LOG_DIR.join(REQUEST_LOG_FILE);
    if let Some(dir) = path.parent()
        && let Err(e) = tokio::fs::create_dir_all(dir).await
    {
        error!("Failed to create log directory {}: {}", dir.display(), e);
        return;
    }

    let logs = store.logs.iter().cloned().collect::<Vec<_>>();
    let Ok(text) = serde_json::to_string_pretty(&logs) else {
        error!("Failed to serialize API request logs");
        return;
    };
    if let Err(e) = tokio::fs::write(&path, text).await {
        error!(
            "Failed to write API request log file {}: {}",
            path.display(),
            e
        );
    }
}

fn count_cache_control_breakpoints(params: &CreateMessageParams) -> usize {
    let system_count = count_cache_controls_in_value(params.system.as_ref());
    let message_count = params
        .messages
        .iter()
        .map(|message| match &message.content {
            MessageContent::Text { .. } => 0,
            MessageContent::Blocks { content } => content
                .iter()
                .filter(|block| {
                    serde_json::to_value(block)
                        .ok()
                        .and_then(|value| value.get("cache_control").cloned())
                        .is_some_and(|value| !value.is_null())
                })
                .count(),
        })
        .sum::<usize>();
    let tool_count = params
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .iter()
                .filter(|tool| tool_has_cache_control(tool))
                .count()
        })
        .unwrap_or(0);
    system_count + message_count + tool_count
}

fn count_cache_controls_in_value(value: Option<&serde_json::Value>) -> usize {
    match value {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter(|item| item.get("cache_control").is_some_and(|v| !v.is_null()))
            .count(),
        Some(serde_json::Value::Object(obj)) => {
            obj.get("cache_control")
                .is_some_and(|value| !value.is_null()) as usize
        }
        _ => 0,
    }
}

fn tool_has_cache_control(tool: &Tool) -> bool {
    serde_json::to_value(tool)
        .ok()
        .and_then(|value| value.get("cache_control").cloned())
        .is_some_and(|value| !value.is_null())
}

fn extract_error_code(error: &str) -> Option<String> {
    for token in error.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if token.len() == 3
            && token.chars().all(|ch| ch.is_ascii_digit())
            && matches!(token.as_bytes()[0], b'4' | b'5')
        {
            return Some(token.to_string());
        }
    }
    None
}

pub fn usage_from_value(value: &serde_json::Value) -> UsageSnapshot {
    let Some(usage) = value.get("usage") else {
        return UsageSnapshot::default();
    };
    UsageSnapshot {
        input_tokens: number(usage, "input_tokens"),
        output_tokens: number(usage, "output_tokens"),
        cache_creation_input_tokens: number(usage, "cache_creation_input_tokens"),
        cache_read_input_tokens: number(usage, "cache_read_input_tokens"),
    }
}

fn number(value: &serde_json::Value, key: &str) -> Option<u64> {
    value
        .get(key)
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n.max(0) as u64)))
}
