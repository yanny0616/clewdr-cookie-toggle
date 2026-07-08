use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use colored::{ColoredString, Colorize};
use tokio::spawn;
use tracing::error;
use wreq::{Client, Proxy};
use wreq_util::Emulation;

use crate::{
    config::{CLEWDR_CONFIG, LOG_DIR},
    error::ClewdrError,
};

/// Helper function to format a boolean value as "Enabled" or "Disabled"
pub fn enabled(flag: bool) -> ColoredString {
    if flag {
        "Enabled".green()
    } else {
        "Disabled".red()
    }
}

/// Helper function to print out JSON to a file in the log directory
///
/// # Arguments
/// * `json` - The JSON object to serialize and output
/// * `file_name` - The name of the file to write in the log directory
pub fn print_out_json(json: impl serde::ser::Serialize, file_name: &str) {
    if CLEWDR_CONFIG.load().no_fs {
        return;
    }
    let text = serde_json::to_string_pretty(&json).unwrap_or_default();
    print_out_text(text, file_name);
}

/// Helper function to print out text to a file in the log directory
///
/// # Arguments
/// * `text` - The text content to write
/// * `file_name` - The name of the file to write in the log directory
pub fn print_out_text(text: String, file_name: &str) {
    let config = CLEWDR_CONFIG.load();
    if config.no_fs {
        return;
    }
    let archive_request_body = should_archive_request_body(file_name);
    let archive_limit = config.request_body_archive_limit;
    let path = LOG_DIR.join(file_name);
    let file_name = file_name.to_owned();
    spawn(async move {
        if let Some(dir) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(dir).await
        {
            error!("Failed to create log directory {}: {}", dir.display(), e);
            return;
        }
        let archive_text = archive_request_body.then(|| text.clone());
        if let Err(e) = tokio::fs::write(&path, text).await {
            error!("Failed to write log file {}: {}", path.display(), e);
        }
        if let Some(text) = archive_text
            && archive_limit > 0
        {
            archive_request_body_dump(&file_name, text, archive_limit).await;
        }
    });
}

fn should_archive_request_body(file_name: &str) -> bool {
    file_name.ends_with("_req.json")
}

async fn archive_request_body_dump(file_name: &str, text: String, limit: usize) {
    let archive_dir = LOG_DIR.join("request_bodies");
    if let Err(e) = tokio::fs::create_dir_all(&archive_dir).await {
        error!(
            "Failed to create request body archive directory {}: {}",
            archive_dir.display(),
            e
        );
        return;
    }

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let archive_name = format!("{millis}_{file_name}");
    let archive_path = archive_dir.join(archive_name);
    if let Err(e) = tokio::fs::write(&archive_path, text).await {
        error!(
            "Failed to write request body archive {}: {}",
            archive_path.display(),
            e
        );
        return;
    }

    cleanup_request_body_archive(&archive_dir, limit).await;
}

async fn cleanup_request_body_archive(archive_dir: &std::path::Path, limit: usize) {
    let Ok(mut entries) = tokio::fs::read_dir(archive_dir).await else {
        return;
    };

    let mut files = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let modified = entry
            .metadata()
            .await
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        files.push((modified, path));
    }

    if files.len() <= limit {
        return;
    }

    files.sort_by_key(|(modified, path)| (*modified, path.clone()));
    let remove_count = files.len().saturating_sub(limit);
    for (_, path) in files.into_iter().take(remove_count) {
        if let Err(e) = tokio::fs::remove_file(&path).await {
            error!(
                "Failed to remove old request body archive {}: {}",
                path.display(),
                e
            );
        }
    }
}

pub fn build_http_client(proxy: Option<&Proxy>) -> Result<Client, wreq::Error> {
    let mut builder = Client::builder()
        .cookie_store(true)
        .emulation(Emulation::Chrome145);
    if let Some(proxy) = proxy {
        builder = builder.proxy(proxy.to_owned());
    }
    builder.build()
}

/// Timezone for the API
pub const TIME_ZONE: &str = "America/New_York";

pub fn forward_response(in_: wreq::Response) -> Result<http::Response<Body>, ClewdrError> {
    let status = in_.status();
    let header = in_.headers().to_owned();
    let stream = in_.bytes_stream();
    let mut res = http::Response::builder().status(status);

    let headers = res.headers_mut().unwrap();
    for (key, value) in header {
        if let Some(key) = key {
            headers.insert(key, value);
        }
    }

    Ok(res.body(Body::from_stream(stream))?)
}
