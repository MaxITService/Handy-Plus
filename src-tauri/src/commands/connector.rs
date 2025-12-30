//! Tauri commands for Connector Manager
//!
//! Commands to control and query the connector server status.

use crate::managers::connector::{ConnectorManager, ConnectorStatus};
use std::sync::Arc;
use tauri::State;

/// Get current connector/extension status
#[tauri::command]
#[specta::specta]
pub fn connector_get_status(manager: State<Arc<ConnectorManager>>) -> ConnectorStatus {
    manager.get_status()
}

/// Check if extension is currently online
#[tauri::command]
#[specta::specta]
pub fn connector_is_online(manager: State<Arc<ConnectorManager>>) -> bool {
    manager.is_online()
}

/// Start the connector server
#[tauri::command]
#[specta::specta]
pub fn connector_start_server(manager: State<Arc<ConnectorManager>>) -> Result<(), String> {
    manager.start_server()
}

/// Stop the connector server
#[tauri::command]
#[specta::specta]
pub fn connector_stop_server(manager: State<Arc<ConnectorManager>>) {
    manager.stop_server()
}

/// Queue a message to be sent to the extension
/// Returns the message ID on success
#[tauri::command]
#[specta::specta]
pub fn connector_queue_message(
    manager: State<Arc<ConnectorManager>>,
    text: String,
) -> Result<String, String> {
    manager.queue_message(&text)
}

/// Cancel a queued message if it hasn't been delivered yet
/// Returns true if message was cancelled, false if not found or already delivered
#[tauri::command]
#[specta::specta]
pub fn connector_cancel_message(
    manager: State<Arc<ConnectorManager>>,
    message_id: String,
) -> Result<bool, String> {
    manager.cancel_queued_message(&message_id)
}
