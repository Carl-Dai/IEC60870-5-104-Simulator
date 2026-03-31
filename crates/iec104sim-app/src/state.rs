use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::slave::SlaveServer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime state for a slave server.
pub struct SlaveServerState {
    pub server: SlaveServer,
    pub log_collector: Arc<LogCollector>,
}

/// Application state holding all active servers.
pub struct AppState {
    pub servers: RwLock<HashMap<String, SlaveServerState>>,
    pub next_server_id: RwLock<u32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
            next_server_id: RwLock::new(1),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// DTOs for API responses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServerInfo {
    pub id: String,
    pub bind_address: String,
    pub port: u16,
    pub state: String,
    pub station_count: usize,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationInfo {
    pub common_address: u16,
    pub name: String,
    pub point_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPointInfo {
    pub ioa: u32,
    pub asdu_type: String,
    pub category: String,
    pub name: String,
    pub comment: String,
    pub value: String,
    pub quality_iv: bool,
    pub timestamp: Option<String>,
}
