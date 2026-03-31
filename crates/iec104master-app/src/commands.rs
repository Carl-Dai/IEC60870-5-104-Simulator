use crate::state::{AppState, ConnectionInfo, MasterConnectionState, ReceivedDataPointInfo};
use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::log_entry::LogEntry;
use iec104sim_core::master::{MasterConfig, MasterConnection, TlsConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// Event Payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ConnectionStateEvent {
    pub id: String,
    pub state: String,
}

// ---------------------------------------------------------------------------
// Connection Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateConnectionRequest {
    pub target_address: String,
    pub port: u16,
    pub common_address: Option<u16>,
    pub timeout_ms: Option<u64>,
    /// TLS configuration
    pub use_tls: Option<bool>,
    pub ca_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub accept_invalid_certs: Option<bool>,
}

#[tauri::command]
pub async fn create_connection(
    state: State<'_, AppState>,
    request: CreateConnectionRequest,
) -> Result<ConnectionInfo, String> {
    let id = {
        let mut counter = state.next_connection_id.write().await;
        let id = format!("conn_{}", *counter);
        *counter += 1;
        id
    };

    let config = MasterConfig {
        target_address: request.target_address.clone(),
        port: request.port,
        common_address: request.common_address.unwrap_or(1),
        timeout_ms: request.timeout_ms.unwrap_or(3000),
        tls: TlsConfig {
            enabled: request.use_tls.unwrap_or(false),
            ca_file: request.ca_file.unwrap_or_default(),
            cert_file: request.cert_file.unwrap_or_default(),
            key_file: request.key_file.unwrap_or_default(),
            accept_invalid_certs: request.accept_invalid_certs.unwrap_or(false),
        },
    };

    let log_collector = Arc::new(LogCollector::new());
    let connection = MasterConnection::new(config.clone())
        .with_log_collector(log_collector.clone());

    let use_tls = config.tls.enabled;
    let info = ConnectionInfo {
        id: id.clone(),
        target_address: config.target_address,
        port: config.port,
        common_address: config.common_address,
        state: format!("{:?}", connection.state().await),
        use_tls,
    };

    state.connections.write().await.insert(
        id,
        MasterConnectionState {
            connection,
            log_collector,
        },
    );

    Ok(info)
}

#[tauri::command]
pub async fn connect_master(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let state_str: String;
    {
        let mut connections = state.connections.write().await;
        let conn = connections
            .get_mut(&id)
            .ok_or_else(|| format!("connection {} not found", id))?;

        conn.connection
            .connect()
            .await
            .map_err(|e| format!("failed to connect: {}", e))?;
        state_str = format!("{:?}", conn.connection.state().await);
    }

    app_handle.emit("connection-state", ConnectionStateEvent {
        id, state: state_str,
    }).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn disconnect_master(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let state_str: String;
    {
        let mut connections = state.connections.write().await;
        let conn = connections
            .get_mut(&id)
            .ok_or_else(|| format!("connection {} not found", id))?;

        conn.connection
            .disconnect()
            .await
            .map_err(|e| format!("failed to disconnect: {}", e))?;
        state_str = format!("{:?}", conn.connection.state().await);
    }

    app_handle.emit("connection-state", ConnectionStateEvent {
        id, state: state_str,
    }).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut connections = state.connections.write().await;
    connections
        .remove(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;
    Ok(())
}

#[tauri::command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, String> {
    let connections = state.connections.read().await;
    let mut result = Vec::new();

    for (id, conn_state) in connections.iter() {
        result.push(ConnectionInfo {
            id: id.clone(),
            target_address: conn_state.connection.config.target_address.clone(),
            port: conn_state.connection.config.port,
            common_address: conn_state.connection.config.common_address,
            state: format!("{:?}", conn_state.connection.state().await),
            use_tls: conn_state.connection.config.tls.enabled,
        });
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// IEC 104 Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn send_interrogation(
    state: State<'_, AppState>,
    id: String,
    common_address: u16,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;

    conn.connection
        .send_interrogation(common_address)
        .await
        .map_err(|e| format!("failed to send GI: {}", e))
}

#[tauri::command]
pub async fn send_clock_sync(
    state: State<'_, AppState>,
    id: String,
    common_address: u16,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;

    conn.connection
        .send_clock_sync(common_address)
        .await
        .map_err(|e| format!("failed to send clock sync: {}", e))
}

#[tauri::command]
pub async fn send_counter_read(
    state: State<'_, AppState>,
    id: String,
    common_address: u16,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;

    conn.connection
        .send_counter_read(common_address)
        .await
        .map_err(|e| format!("failed to send counter read: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ControlCommandRequest {
    pub connection_id: String,
    pub ioa: u32,
    pub common_address: u16,
    pub command_type: String,
    pub value: String,
    pub select: Option<bool>,
}

#[tauri::command]
pub async fn send_control_command(
    state: State<'_, AppState>,
    request: ControlCommandRequest,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let select = request.select.unwrap_or(false);
    let ca = request.common_address;

    let result = match request.command_type.as_str() {
        "single" => {
            let value = request.value.parse::<bool>()
                .or_else(|_| match request.value.as_str() {
                    "1" | "true" | "ON" => Ok(true),
                    "0" | "false" | "OFF" => Ok(false),
                    _ => Err(format!("invalid bool: {}", request.value)),
                })
                .map_err(|e| format!("{}", e))?;
            conn.connection.send_single_command(request.ioa, value, select, ca).await
                .map_err(|e| format!("failed to send command: {}", e))
        }
        "double" => {
            let value = request.value.parse::<u8>().map_err(|e| format!("{}", e))?;
            conn.connection.send_double_command(request.ioa, value, select, ca).await
                .map_err(|e| format!("failed to send command: {}", e))
        }
        "setpoint_float" => {
            let value = request.value.parse::<f32>().map_err(|e| format!("{}", e))?;
            conn.connection.send_setpoint_float(request.ioa, value, ca).await
                .map_err(|e| format!("failed to send command: {}", e))
        }
        _ => Err(format!("unknown command type: {}", request.command_type)),
    };
    result
}

// ---------------------------------------------------------------------------
// Data Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_received_data(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ReceivedDataPointInfo>, String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;

    let data = conn.connection.received_data.read().await;
    let result: Vec<ReceivedDataPointInfo> = data
        .all_sorted()
        .iter()
        .map(|p| ReceivedDataPointInfo {
            ioa: p.ioa,
            asdu_type: p.asdu_type.name().to_string(),
            category: p.asdu_type.category().name().to_string(),
            value: p.value.display(),
            quality_iv: p.quality.iv,
            timestamp: p.timestamp.map(|t| t.format("%H:%M:%S%.3f").to_string()),
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Log Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<LogEntry>, String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    Ok(conn.log_collector.get_all().await)
}

#[tauri::command]
pub async fn clear_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    conn.log_collector.clear().await;
    Ok(())
}

#[tauri::command]
pub async fn export_logs_csv(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    Ok(conn.log_collector.export_csv().await)
}
