use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::master::MasterConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime state for a master connection.
pub struct MasterConnectionState {
    pub connection: MasterConnection,
    pub log_collector: Arc<LogCollector>,
}

/// Application state holding all active master connections.
pub struct AppState {
    pub connections: RwLock<HashMap<String, MasterConnectionState>>,
    pub next_connection_id: RwLock<u32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            next_connection_id: RwLock::new(1),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConnectionInfo {
    pub id: String,
    pub target_address: String,
    pub port: u16,
    pub common_address: u16,
    pub state: String,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedDataPointInfo {
    pub ioa: u32,
    pub asdu_type: String,
    pub category: String,
    pub value: String,
    pub quality_iv: bool,
    pub timestamp: Option<String>,
    pub update_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalDataResponse {
    pub seq: u64,
    pub total_count: usize,
    pub points: Vec<ReceivedDataPointInfo>,
}
