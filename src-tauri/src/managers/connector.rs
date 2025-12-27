//! Connector Manager - HTTP server for Chrome extension communication
//!
//! This module provides an HTTP server that allows the Handy Chrome extension
//! to poll for messages. It tracks the connection status based on polling activity.

use crate::settings::{default_connector_password, get_settings, write_settings};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Request, Response, Server};

/// Default server port (same as test-server.ps1)
const DEFAULT_PORT: u16 = 63155;
/// Timeout in milliseconds - if no poll for this duration, consider disconnected
const POLL_TIMEOUT_MS: i64 = 10_000;
/// Keepalive interval in milliseconds
const KEEPALIVE_INTERVAL_MS: i64 = 15_000;
/// Maximum messages to keep in queue
const MAX_MESSAGES: usize = 100;
/// How long to keep blobs available for download (5 minutes)
const BLOB_EXPIRY_MS: i64 = 300_000;

/// Extension connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStatus {
    /// Extension is actively polling
    Online,
    /// Extension has not polled recently
    Offline,
    /// Server is starting up, status unknown
    Unknown,
}

/// Status info returned to frontend
#[derive(Debug, Clone, Serialize, Type)]
pub struct ConnectorStatus {
    pub status: ExtensionStatus,
    /// Last time extension polled (Unix timestamp in ms), 0 if never
    pub last_poll_at: i64,
    /// Server is running
    pub server_running: bool,
    /// Port server is listening on
    pub port: u16,
}

/// A message in the queue to be sent to extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: String,
    pub ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<BundleAttachment>>,
}

/// Attachment info for bundle messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAttachment {
    #[serde(rename = "attId")]
    pub att_id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub fetch: BundleFetch,
}

/// Fetch info for attachments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFetch {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// A blob stored for serving to extension
#[derive(Debug, Clone)]
pub struct PendingBlob {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub expires_at: i64,
}

/// Configuration sent to extension
#[derive(Debug, Clone, Serialize)]
struct ExtensionConfig {
    /// URL to auto-open when no tab is bound (empty string = disabled)
    #[serde(rename = "autoOpenTabUrl")]
    auto_open_tab_url: Option<String>,
}

/// Response format for GET /messages
#[derive(Debug, Clone, Serialize)]
struct MessagesResponse {
    cursor: i64,
    messages: Vec<QueuedMessage>,
    config: ExtensionConfig,
    /// New password if auto-generated (extension should save this)
    #[serde(rename = "passwordUpdate", skip_serializing_if = "Option::is_none")]
    password_update: Option<String>,
}

/// POST body from extension (ack or message)
#[derive(Debug, Clone, Deserialize)]
struct PostBody {
    #[serde(default)]
    text: Option<String>,
    #[serde(rename = "type", default)]
    msg_type: Option<String>,
}

/// Internal state shared between threads
struct ConnectorState {
    /// Queue of messages waiting to be picked up by extension
    messages: VecDeque<QueuedMessage>,
    /// Timestamp of last keepalive sent
    last_keepalive: i64,
    /// Blobs stored for extension to download (attId -> blob data)
    blobs: HashMap<String, PendingBlob>,
    /// Set of message IDs that have been delivered (for deduplication)
    delivered_ids: std::collections::HashSet<String>,
}

pub struct ConnectorManager {
    app_handle: AppHandle,
    /// Timestamp of last poll from extension (atomic for lock-free access)
    last_poll_at: Arc<AtomicI64>,
    /// Whether server is running
    server_running: Arc<AtomicBool>,
    /// Port server is listening on
    port: RwLock<u16>,
    /// Shared state for message queue
    state: Arc<Mutex<ConnectorState>>,
    /// Flag to stop the server
    stop_flag: Arc<AtomicBool>,
}

impl ConnectorManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let settings = get_settings(app_handle);
        maybe_migrate_legacy_connector_password(app_handle, &settings);

        let port = if settings.connector_port > 0 {
            settings.connector_port
        } else {
            DEFAULT_PORT
        };

        let manager = Self {
            app_handle: app_handle.clone(),
            last_poll_at: Arc::new(AtomicI64::new(0)),
            server_running: Arc::new(AtomicBool::new(false)),
            port: RwLock::new(port),
            state: Arc::new(Mutex::new(ConnectorState {
                messages: VecDeque::new(),
                last_keepalive: 0,
                blobs: HashMap::new(),
                delivered_ids: std::collections::HashSet::new(),
            })),
            stop_flag: Arc::new(AtomicBool::new(false)),
        };

        Ok(manager)
    }

    /// Start the HTTP server in a background thread
    pub fn start_server(&self) -> Result<(), String> {
        if self.server_running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        let port = *self.port.read().unwrap();
        let server = Self::try_bind_server(port)?;

        self.server_running.store(true, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);

        let app_handle = self.app_handle.clone();
        let state = self.state.clone();
        let stop_flag = self.stop_flag.clone();
        let last_poll_at = Arc::clone(&self.last_poll_at);
        let server_running = Arc::clone(&self.server_running);

        thread::spawn(move || {
            info!("Connector server started on port {}", port);

            // Emit initial status
            let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Unknown);

            let mut was_online = false;

            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                // Handle incoming requests with timeout
                match server.recv_timeout(Duration::from_millis(500)) {
                    Ok(Some(request)) => {
                        Self::handle_request(
                            request,
                            &state,
                            &last_poll_at,
                            &app_handle,
                            &mut was_online,
                        );
                    }
                    Ok(None) => {
                        // Timeout, check status and keepalive
                    }
                    Err(e) => {
                        error!("Server error: {}", e);
                        break;
                    }
                }

                // Check connection status
                let now_ms = now_ms();
                let last_poll = last_poll_at.load(Ordering::SeqCst);

                if last_poll > 0 {
                    let is_online = (now_ms - last_poll) < POLL_TIMEOUT_MS;

                    if is_online != was_online {
                        let status = if is_online {
                            ExtensionStatus::Online
                        } else {
                            ExtensionStatus::Offline
                        };
                        info!("Extension status changed: {:?}", status);
                        let _ = app_handle.emit("extension-status-changed", status);
                        was_online = is_online;
                    }
                }

                // Send keepalive if needed
                Self::maybe_send_keepalive(&state, now_ms);
            }

            server_running.store(false, Ordering::SeqCst);
            info!("Connector server stopped");
        });

        Ok(())
    }

    /// Minimum allowed port number (1024 = first non-privileged port)
    const MIN_PORT: u16 = 1024;

    /// Try to bind to the specified port (exact port only, no fallback)
    fn try_bind_server(port: u16) -> Result<Server, String> {
        // Validate port range
        if port < Self::MIN_PORT {
            return Err(format!(
                "Port {} is not allowed. Please use a port number of {} or higher.",
                port,
                Self::MIN_PORT
            ));
        }

        let addr = format!("127.0.0.1:{}", port);

        Server::http(&addr).map_err(|e| {
            error!("Failed to bind connector server to port {}: {}", port, e);
            format!(
                "Port {} is already in use or unavailable. Please choose a different port.",
                port
            )
        })
    }

    /// Handle an incoming HTTP request
    fn handle_request(
        mut request: Request,
        state: &Arc<Mutex<ConnectorState>>,
        last_poll_at: &Arc<AtomicI64>,
        app_handle: &AppHandle,
        was_online: &mut bool,
    ) {
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        let method = request.method().clone();

        // Add CORS headers to all responses
        let cors_headers = vec![
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            Header::from_bytes(
                &b"Access-Control-Allow-Headers"[..],
                &b"Authorization, Content-Type"[..],
            )
            .unwrap(),
            Header::from_bytes(
                &b"Access-Control-Allow-Methods"[..],
                &b"GET, POST, OPTIONS"[..],
            )
            .unwrap(),
            Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap(),
        ];

        match (&method, path.as_str()) {
            (Method::Options, _) => {
                // CORS preflight - no auth needed
                let mut response = Response::empty(204);
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }

            (Method::Get, "/messages") => {
                // Auth check for messages endpoint
                let settings = get_settings(app_handle);
                if !validate_auth(
                    &request,
                    &settings.connector_password,
                    settings.connector_pending_password.as_deref(),
                ) {
                    Self::respond_unauthorized(request, cors_headers);
                    return;
                }

                // Extension is polling for messages
                let now = now_ms();
                let old_poll = last_poll_at.swap(now, Ordering::SeqCst);

                // If this is first poll or we were offline, emit online status
                if old_poll == 0 || (now - old_poll) >= POLL_TIMEOUT_MS {
                    info!("Extension connected (polling started)");
                    let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Online);
                    *was_online = true;
                }

                // Parse cursor from query string
                let cursor = request
                    .url()
                    .split('?')
                    .nth(1)
                    .and_then(|query| {
                        query
                            .split('&')
                            .find(|p| p.starts_with("since="))
                            .and_then(|p| p.strip_prefix("since="))
                            .and_then(|v| v.parse::<i64>().ok())
                    })
                    .unwrap_or(0);

                // Get messages newer than or equal to cursor, excluding already-delivered IDs
                let (messages, next_cursor) = {
                    let mut state_guard = state.lock().unwrap();
                    let filtered: Vec<_> = state_guard
                        .messages
                        .iter()
                        .filter(|m| m.ts >= cursor && !state_guard.delivered_ids.contains(&m.id))
                        .cloned()
                        .collect();

                    // Mark these messages as delivered
                    for msg in &filtered {
                        state_guard.delivered_ids.insert(msg.id.clone());
                    }

                    // Clean up old delivered IDs (keep only IDs from messages still in queue)
                    let current_ids: std::collections::HashSet<_> =
                        state_guard.messages.iter().map(|m| m.id.clone()).collect();
                    state_guard
                        .delivered_ids
                        .retain(|id| current_ids.contains(id));

                    let next = filtered.last().map(|m| m.ts).unwrap_or(cursor);
                    (filtered, next)
                };

                // Check if we need to generate a new password (first connection with default password)
                let password_update = maybe_generate_new_password(app_handle);

                // Get config from settings (re-read in case password was updated)
                let settings = get_settings(app_handle);
                let auto_open_url = if settings.connector_auto_open_enabled
                    && !settings.connector_auto_open_url.is_empty()
                {
                    Some(settings.connector_auto_open_url.clone())
                } else {
                    None
                };

                let response_body = MessagesResponse {
                    cursor: next_cursor,
                    messages,
                    config: ExtensionConfig {
                        auto_open_tab_url: auto_open_url,
                    },
                    password_update,
                };

                let json = serde_json::to_string(&response_body).unwrap_or_default();
                let mut response = Response::from_string(json);
                response.add_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                );
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }

            (Method::Post, "/messages") => {
                // Auth check for messages endpoint
                let settings = get_settings(app_handle);
                if !validate_auth(
                    &request,
                    &settings.connector_password,
                    settings.connector_pending_password.as_deref(),
                ) {
                    Self::respond_unauthorized(request, cors_headers);
                    return;
                }

                // Extension sending status/ack
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);

                if let Ok(post_body) = serde_json::from_str::<PostBody>(&body) {
                    if post_body.msg_type.as_deref() == Some("keepalive_ack") {
                        debug!("Received keepalive ack from extension");
                    } else if post_body.msg_type.as_deref() == Some("password_ack") {
                        // Extension acknowledged receiving the new password - commit it
                        commit_pending_password(app_handle);
                    } else if let Some(text) = post_body.text {
                        debug!("Received message from extension: {}", text);
                    }
                }

                // Update last poll time on POST too
                last_poll_at.store(now_ms(), Ordering::SeqCst);

                let json = r#"{"ok":true}"#;
                let mut response = Response::from_string(json);
                response.add_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                );
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }

            (Method::Get, blob_path) if blob_path.starts_with("/blob/") => {
                // Auth check for blob endpoint
                let settings = get_settings(app_handle);
                if !validate_auth(
                    &request,
                    &settings.connector_password,
                    settings.connector_pending_password.as_deref(),
                ) {
                    Self::respond_unauthorized(request, cors_headers);
                    return;
                }

                // Serve blob data for attachments
                let att_id = blob_path.strip_prefix("/blob/").unwrap_or("");

                let blob_data = {
                    let mut state_guard = state.lock().unwrap();
                    let now = now_ms();

                    // Clean up expired blobs
                    state_guard.blobs.retain(|_, blob| blob.expires_at > now);

                    // Get the requested blob
                    state_guard.blobs.get(att_id).cloned()
                };

                match blob_data {
                    Some(blob) => {
                        debug!(
                            "Serving blob {} ({} bytes, {})",
                            att_id,
                            blob.data.len(),
                            blob.mime_type
                        );
                        let mut response = Response::from_data(blob.data);
                        response.add_header(
                            Header::from_bytes(&b"Content-Type"[..], blob.mime_type.as_bytes())
                                .unwrap(),
                        );
                        for header in cors_headers {
                            response.add_header(header);
                        }
                        let _ = request.respond(response);
                    }
                    None => {
                        debug!("Blob not found or expired: {}", att_id);
                        let mut response =
                            Response::from_string("Blob not found").with_status_code(404);
                        for header in cors_headers {
                            response.add_header(header);
                        }
                        let _ = request.respond(response);
                    }
                }
            }

            _ => {
                // 404 for unknown paths
                let mut response = Response::from_string("Not Found").with_status_code(404);
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }
        }
    }

    /// Send keepalive message if enough time has passed
    fn maybe_send_keepalive(state: &Arc<Mutex<ConnectorState>>, now_ms: i64) {
        let mut state_guard = state.lock().unwrap();

        if now_ms - state_guard.last_keepalive > KEEPALIVE_INTERVAL_MS {
            state_guard.last_keepalive = now_ms;

            let keepalive = QueuedMessage {
                id: uuid_simple(),
                msg_type: "keepalive".to_string(),
                text: "keepalive".to_string(),
                ts: now_ms,
                attachments: None,
            };

            state_guard.messages.push_back(keepalive);

            // Trim old messages
            while state_guard.messages.len() > MAX_MESSAGES {
                state_guard.messages.pop_front();
            }
        }
    }

    /// Stop the HTTP server
    pub fn stop_server(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Update the port and restart the server if it's running
    pub fn restart_on_port(&self, new_port: u16) -> Result<(), String> {
        // Update the stored port
        {
            let mut port = self.port.write().unwrap();
            *port = new_port;
        }

        // If server is running, restart it on the new port
        if self.server_running.load(Ordering::SeqCst) {
            info!("Restarting connector server on new port {}", new_port);
            self.stop_server();

            // Wait for server to stop (with timeout)
            let start = std::time::Instant::now();
            while self.server_running.load(Ordering::SeqCst) {
                if start.elapsed() > Duration::from_secs(2) {
                    return Err("Timeout waiting for server to stop".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            // Reset last poll so status goes to Unknown
            self.last_poll_at.store(0, Ordering::SeqCst);

            // Start on new port
            self.start_server()?;
        }

        Ok(())
    }

    /// Queue a message to be sent to the extension
    pub fn queue_message(&self, text: &str) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("Message is empty".to_string());
        }

        let msg = QueuedMessage {
            id: uuid_simple(),
            msg_type: "text".to_string(),
            text: trimmed.to_string(),
            ts: now_ms(),
            attachments: None,
        };

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.messages.push_back(msg);

        // Trim old messages
        while state.messages.len() > MAX_MESSAGES {
            state.messages.pop_front();
        }

        Ok(())
    }

    /// Queue a bundle message with an image attachment
    pub fn queue_bundle_message(&self, text: &str, image_path: &PathBuf) -> Result<(), String> {
        // Read the image file
        let data =
            std::fs::read(image_path).map_err(|e| format!("Failed to read image file: {}", e))?;

        // Determine MIME type from extension
        let extension = image_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
        let mime_type = match extension.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            _ => "image/png",
        };

        let filename = image_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let file_size = data.len() as u64;
        let att_id = uuid_simple();
        let now = now_ms();
        let expires_at = now + BLOB_EXPIRY_MS;

        // Get settings for port and auth token
        let settings = get_settings(&self.app_handle);
        let port = *self.port.read().unwrap();
        let fetch_url = format!("http://127.0.0.1:{}/blob/{}", port, att_id);

        // Include auth header so extension can fetch blobs (blob endpoint requires auth)
        let mut fetch_headers = HashMap::new();
        fetch_headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", settings.connector_password),
        );

        // Create the attachment
        let attachment = BundleAttachment {
            att_id: att_id.clone(),
            kind: "image".to_string(),
            filename,
            mime: Some(mime_type.to_string()),
            size: Some(file_size),
            fetch: BundleFetch {
                url: fetch_url,
                method: Some("GET".to_string()),
                headers: Some(fetch_headers),
                expires_at: Some(expires_at),
            },
        };

        // Store the blob
        let pending_blob = PendingBlob {
            data,
            mime_type: mime_type.to_string(),
            expires_at,
        };

        // Create the bundle message
        let msg = QueuedMessage {
            id: uuid_simple(),
            msg_type: "bundle".to_string(),
            text: text.trim().to_string(),
            ts: now,
            attachments: Some(vec![attachment]),
        };

        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        // Store the blob for later retrieval
        state.blobs.insert(att_id, pending_blob);

        // Queue the message
        state.messages.push_back(msg);

        // Trim old messages
        while state.messages.len() > MAX_MESSAGES {
            state.messages.pop_front();
        }

        // Clean up expired blobs
        let now = now_ms();
        state.blobs.retain(|_, blob| blob.expires_at > now);

        debug!(
            "Queued bundle message with image attachment ({} bytes)",
            file_size
        );
        Ok(())
    }

    /// Get current connection status
    pub fn get_status(&self) -> ConnectorStatus {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        let now = now_ms();
        let server_running = self.server_running.load(Ordering::SeqCst);
        let port = *self.port.read().unwrap();

        let status = if !server_running {
            ExtensionStatus::Unknown
        } else if last_poll == 0 {
            ExtensionStatus::Unknown
        } else if (now - last_poll) < POLL_TIMEOUT_MS {
            ExtensionStatus::Online
        } else {
            ExtensionStatus::Offline
        };

        ConnectorStatus {
            status,
            last_poll_at: last_poll,
            server_running,
            port,
        }
    }

    /// Check if extension is currently online
    pub fn is_online(&self) -> bool {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        if last_poll == 0 {
            return false;
        }
        (now_ms() - last_poll) < POLL_TIMEOUT_MS
    }

    /// Send 401 Unauthorized response
    fn respond_unauthorized(request: Request, cors_headers: Vec<Header>) {
        let mut response = Response::from_string("Unauthorized").with_status_code(401);
        response.add_header(Header::from_bytes(&b"WWW-Authenticate"[..], &b"Bearer"[..]).unwrap());
        for header in cors_headers {
            response.add_header(header);
        }
        let _ = request.respond(response);
    }
}

/// Migrate legacy connector password state (pre two-phase-commit) into a recoverable state.
///
/// Older versions could auto-generate and persist a random password without the extension ever
/// receiving it, locking the extension out. If we detect that scenario, we temporarily revert the
/// configured password back to the default and stash the existing password as "pending", so the
/// extension can reconnect using the default and then acknowledge the pending password.
fn maybe_migrate_legacy_connector_password(
    app_handle: &AppHandle,
    settings: &crate::settings::AppSettings,
) {
    if settings.connector_password_user_set || settings.connector_pending_password.is_some() {
        return;
    }

    let default_password = default_connector_password();
    if settings.connector_password.is_empty() || settings.connector_password == default_password {
        return;
    }

    if !is_probably_autogenerated_password(&settings.connector_password) {
        return;
    }

    info!(
        "Detected legacy auto-generated connector password; migrating to two-phase commit handshake"
    );

    let mut new_settings = settings.clone();
    new_settings.connector_pending_password = Some(settings.connector_password.clone());
    new_settings.connector_password = default_password;
    write_settings(app_handle, new_settings);
}

fn is_probably_autogenerated_password(password: &str) -> bool {
    password.len() == 32
        && password
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Get current Unix timestamp in milliseconds
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Generate a simple UUID (hex string without dashes)
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", ts)
}

/// Validate Authorization header against expected password
/// Also accepts the pending password during two-phase commit transition
fn validate_auth(
    request: &Request,
    expected_password: &str,
    pending_password: Option<&str>,
) -> bool {
    // If no password configured, reject all requests
    if expected_password.is_empty() {
        return false;
    }

    // Check Authorization: Bearer <password>
    for header in request.headers() {
        if header.field.equiv("Authorization") {
            let value = header.value.as_str();
            if let Some(token) = value.strip_prefix("Bearer ") {
                // Accept current password
                if constant_time_eq(token.as_bytes(), expected_password.as_bytes()) {
                    return true;
                }
                // Also accept pending password during transition
                if let Some(pending) = pending_password {
                    if constant_time_eq(token.as_bytes(), pending.as_bytes()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Generate a secure random password (32 hex characters)
fn generate_secure_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Combine multiple entropy sources for randomness
    let ts_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Use process id and thread id for additional entropy
    let pid = std::process::id();
    let thread_id = format!("{:?}", std::thread::current().id());

    // Create a hash-like combination
    let seed = format!(
        "{}{}{}{}",
        ts_nanos,
        pid,
        thread_id,
        ts_nanos.wrapping_mul(0x517cc1b727220a95)
    );

    // Generate 32 hex characters from the hash
    let mut result = String::with_capacity(32);
    let bytes = seed.as_bytes();
    let mut acc: u64 = 0;
    for (i, &b) in bytes.iter().enumerate() {
        acc = acc.wrapping_add((b as u64).wrapping_mul((i as u64).wrapping_add(1)));
        acc = acc.wrapping_mul(0x517cc1b727220a95);
    }

    // Generate 4 chunks of 8 hex chars each
    for i in 0..4 {
        let chunk = acc
            .wrapping_mul((i + 1) as u64)
            .wrapping_add(ts_nanos as u64);
        result.push_str(&format!("{:08x}", chunk as u32));
    }

    result
}

/// Check if we should generate a new password and do so if needed.
/// Returns Some(new_password) if a new password was generated or is pending, None otherwise.
/// Uses two-phase commit: password is stored as "pending" until extension acknowledges.
fn maybe_generate_new_password(app_handle: &AppHandle) -> Option<String> {
    let settings = get_settings(app_handle);

    // If there's already a pending password, return it (extension needs to ack)
    if let Some(ref pending) = settings.connector_pending_password {
        debug!("Returning existing pending password for extension to acknowledge");
        return Some(pending.clone());
    }

    // Only generate if:
    // 1. Password has not been explicitly set by user
    // 2. Password is still the default
    if !settings.connector_password_user_set
        && settings.connector_password == default_connector_password()
    {
        let new_password = generate_secure_password();
        info!("Generating new secure connector password (first connection with default) - awaiting acknowledgement");

        // Store as pending - NOT committed until extension acknowledges
        let mut new_settings = settings.clone();
        new_settings.connector_pending_password = Some(new_password.clone());
        // Note: connector_password stays as default until ack received
        write_settings(app_handle, new_settings);

        Some(new_password)
    } else {
        None
    }
}

/// Commit the pending password after extension acknowledges receipt.
/// This completes the two-phase commit for password update.
fn commit_pending_password(app_handle: &AppHandle) {
    let settings = get_settings(app_handle);

    if let Some(ref pending) = settings.connector_pending_password {
        info!("Extension acknowledged password - committing new password");

        let mut new_settings = settings.clone();
        new_settings.connector_password = pending.clone();
        new_settings.connector_pending_password = None;
        // Note: connector_password_user_set stays false (auto-generated)
        write_settings(app_handle, new_settings);
    } else {
        debug!("Received password_ack but no pending password to commit");
    }
}
