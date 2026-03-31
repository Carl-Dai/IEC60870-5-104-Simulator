use crate::data_point::InformationObjectDef;
use crate::master::TlsConfig;
use crate::slave::SlaveTlsConfig;
use serde::{Deserialize, Serialize};

/// Configuration for a slave server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveServerConfig {
    pub bind_address: String,
    pub port: u16,
    #[serde(default)]
    pub tls: SlaveTlsConfig,
    pub stations: Vec<StationConfig>,
}

/// Configuration for a station within a slave server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationConfig {
    pub common_address: u16,
    pub name: String,
    pub data_points: Vec<InformationObjectDef>,
}

/// Configuration for a master connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionConfig {
    pub target_address: String,
    pub port: u16,
    pub common_address: u16,
    pub timeout_ms: u64,
    #[serde(default)]
    pub tls: TlsConfig,
}

impl Default for MasterConnectionConfig {
    fn default() -> Self {
        Self {
            target_address: "127.0.0.1".to_string(),
            port: 2404,
            common_address: 1,
            timeout_ms: 3000,
            tls: TlsConfig::default(),
        }
    }
}

/// Full app state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedAppState {
    pub version: u32,
    pub servers: Vec<SlaveServerConfig>,
}

/// Full master app state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMasterState {
    pub version: u32,
    pub connections: Vec<MasterConnectionConfig>,
}
