use crate::data_point::{DataPoint, DataPointMap, DataPointValue, InformationObjectDef};
use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FrameLabel, LogEntry};
use crate::types::{AsduTypeId, DataCategory};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// TLS Configuration (Slave / Server-side)
// ---------------------------------------------------------------------------

/// TLS configuration for a slave server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlaveTlsConfig {
    /// Enable TLS
    pub enabled: bool,
    /// Path to server certificate file (PEM format)
    #[serde(default)]
    pub cert_file: String,
    /// Path to server private key file (PEM format)
    #[serde(default)]
    pub key_file: String,
    /// Path to CA certificate file (PEM) for client certificate verification (mTLS)
    #[serde(default)]
    pub ca_file: String,
    /// Require client certificate (mutual TLS)
    #[serde(default)]
    pub require_client_cert: bool,
}

// ---------------------------------------------------------------------------
// Stream Abstraction
// ---------------------------------------------------------------------------

/// A stream that can be either plain TCP or TLS-wrapped (server-side).
enum SlaveStream {
    Plain(TcpStream),
    Tls(native_tls::TlsStream<TcpStream>),
}

impl Read for SlaveStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            SlaveStream::Plain(s) => s.read(buf),
            SlaveStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for SlaveStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            SlaveStream::Plain(s) => s.write(buf),
            SlaveStream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            SlaveStream::Plain(s) => s.flush(),
            SlaveStream::Tls(s) => s.flush(),
        }
    }
}

impl SlaveStream {
    fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        match self {
            SlaveStream::Plain(s) => s.set_nonblocking(nonblocking),
            SlaveStream::Tls(s) => s.get_ref().set_nonblocking(nonblocking),
        }
    }

    fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            SlaveStream::Plain(s) => s.set_read_timeout(dur),
            SlaveStream::Tls(s) => s.get_ref().set_read_timeout(dur),
        }
    }
}

/// A station within the slave server (analogous to SlaveDevice).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    /// Common Address (1..65534)
    pub common_address: u16,
    /// User-defined name
    pub name: String,
    /// Data points (keyed by IOA)
    pub data_points: DataPointMap,
    /// Information object definitions (metadata for UI)
    pub object_defs: Vec<InformationObjectDef>,
}

impl Station {
    pub fn new(common_address: u16, name: impl Into<String>) -> Self {
        Self {
            common_address,
            name: name.into(),
            data_points: DataPointMap::new(),
            object_defs: Vec::new(),
        }
    }

    /// Create a station with default data points pre-filled.
    pub fn with_default_points(
        common_address: u16,
        name: impl Into<String>,
        count_per_category: u32,
    ) -> Self {
        let mut station = Self::new(common_address, name);
        let mut ioa = 1u32;

        // Add points for each monitor category
        let categories = [
            (AsduTypeId::MSpNa1, DataCategory::SinglePoint),
            (AsduTypeId::MDpNa1, DataCategory::DoublePoint),
            (AsduTypeId::MMeNa1, DataCategory::NormalizedMeasured),
            (AsduTypeId::MMeNb1, DataCategory::ScaledMeasured),
            (AsduTypeId::MMeNc1, DataCategory::FloatMeasured),
            (AsduTypeId::MItNa1, DataCategory::IntegratedTotals),
        ];

        for (asdu_type, category) in &categories {
            for _ in 0..count_per_category {
                let def = InformationObjectDef {
                    ioa,
                    asdu_type: *asdu_type,
                    category: *category,
                    name: String::new(),
                    comment: String::new(),
                };
                let point = DataPoint::new(ioa, *asdu_type);
                station.data_points.insert(point);
                station.object_defs.push(def);
                ioa += 1;
            }
        }

        station
    }

    /// Create a station with random data point values.
    pub fn with_random_points(
        common_address: u16,
        name: impl Into<String>,
        count_per_category: u32,
    ) -> Self {
        use rand::Rng;
        let mut station = Self::with_default_points(common_address, name, count_per_category);
        let mut rng = rand::thread_rng();

        for point in station.data_points.points.values_mut() {
            point.value = match point.asdu_type.category() {
                DataCategory::SinglePoint => DataPointValue::SinglePoint { value: rng.gen() },
                DataCategory::DoublePoint => DataPointValue::DoublePoint { value: rng.gen_range(1..=2) },
                DataCategory::NormalizedMeasured => DataPointValue::Normalized { value: rng.gen_range(-1.0..1.0) },
                DataCategory::ScaledMeasured => DataPointValue::Scaled { value: rng.gen_range(-1000..1000) },
                DataCategory::FloatMeasured => DataPointValue::ShortFloat { value: rng.gen_range(-100.0..100.0) },
                DataCategory::IntegratedTotals => DataPointValue::IntegratedTotal {
                    value: rng.gen_range(0..10000),
                    carry: false,
                    sequence: 0,
                },
                _ => DataPointValue::default_for(point.asdu_type),
            };
        }

        station
    }

    /// Add a data point definition and its runtime data.
    pub fn add_point(&mut self, def: InformationObjectDef) -> Result<(), SlaveError> {
        if self.data_points.contains(def.ioa) {
            return Err(SlaveError::DuplicateIoa(def.ioa));
        }
        let point = DataPoint::new(def.ioa, def.asdu_type);
        self.data_points.insert(point);
        self.object_defs.push(def);
        Ok(())
    }

    /// Remove a data point by IOA.
    pub fn remove_point(&mut self, ioa: u32) -> Result<(), SlaveError> {
        if !self.data_points.contains(ioa) {
            return Err(SlaveError::IoaNotFound(ioa));
        }
        self.data_points.remove(ioa);
        self.object_defs.retain(|d| d.ioa != ioa);
        Ok(())
    }
}

/// Running state of a slave server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Stopped,
    Running,
}

/// Transport configuration for a slave server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveTransportConfig {
    pub bind_address: String,
    pub port: u16,
    /// TLS configuration (optional)
    #[serde(default)]
    pub tls: SlaveTlsConfig,
}

impl Default for SlaveTransportConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 2404,
            tls: SlaveTlsConfig::default(),
        }
    }
}

/// Shared stations accessible by all connections.
pub type SharedStations = Arc<RwLock<HashMap<u16, Station>>>;

/// The IEC 104 slave server (analogous to SlaveConnection).
pub struct SlaveServer {
    pub transport: SlaveTransportConfig,
    pub stations: SharedStations,
    pub log_collector: Option<Arc<LogCollector>>,
    state: ServerState,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SlaveServer {
    pub fn new(transport: SlaveTransportConfig) -> Self {
        Self {
            transport,
            stations: Arc::new(RwLock::new(HashMap::new())),
            log_collector: None,
            state: ServerState::Stopped,
            shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            server_handle: None,
        }
    }

    pub fn with_log_collector(mut self, collector: Arc<LogCollector>) -> Self {
        self.log_collector = Some(collector);
        self
    }

    pub fn state(&self) -> ServerState {
        self.state
    }

    /// Add a station.
    pub async fn add_station(&self, station: Station) -> Result<(), SlaveError> {
        let mut stations = self.stations.write().await;
        if stations.contains_key(&station.common_address) {
            return Err(SlaveError::DuplicateStation(station.common_address));
        }
        stations.insert(station.common_address, station);
        Ok(())
    }

    /// Remove a station by common address.
    pub async fn remove_station(&self, ca: u16) -> Result<Station, SlaveError> {
        let mut stations = self.stations.write().await;
        stations
            .remove(&ca)
            .ok_or(SlaveError::StationNotFound(ca))
    }

    /// Start the IEC 104 TCP server.
    ///
    /// This implements a basic IEC 104 server that accepts TCP connections
    /// on the configured port and handles STARTDT, general interrogation,
    /// and control commands.
    pub async fn start(&mut self) -> Result<(), SlaveError> {
        if self.state == ServerState::Running {
            return Err(SlaveError::AlreadyRunning);
        }

        let addr = format!("{}:{}", self.transport.bind_address, self.transport.port);
        let listener = TcpListener::bind(&addr)
            .map_err(|e| SlaveError::BindError(format!("Failed to bind {}: {}", addr, e)))?;
        listener.set_nonblocking(true)
            .map_err(|e| SlaveError::BindError(format!("Failed to set non-blocking: {}", e)))?;

        // Build TLS acceptor if TLS is enabled
        let tls_acceptor: Option<Arc<native_tls::TlsAcceptor>> = if self.transport.tls.enabled {
            let tls_config = &self.transport.tls;
            let cert_pem = std::fs::read(&tls_config.cert_file)
                .map_err(|e| SlaveError::TlsError(format!("读取服务器证书失败 {}: {}", tls_config.cert_file, e)))?;
            let key_pem = std::fs::read(&tls_config.key_file)
                .map_err(|e| SlaveError::TlsError(format!("读取服务器密钥失败 {}: {}", tls_config.key_file, e)))?;

            let identity = native_tls::Identity::from_pkcs8(&cert_pem, &key_pem)
                .map_err(|e| SlaveError::TlsError(format!("加载服务器身份失败: {}", e)))?;

            let mut builder = native_tls::TlsAcceptor::builder(identity);
            builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));

            let acceptor = builder.build()
                .map_err(|e| SlaveError::TlsError(format!("创建 TLS 接受器失败: {}", e)))?;
            Some(Arc::new(acceptor))
        } else {
            None
        };

        let shutdown_flag = self.shutdown_flag.clone();
        shutdown_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        let stations = self.stations.clone();
        let log_collector = self.log_collector.clone();
        let is_tls = self.transport.tls.enabled;

        let handle = tokio::spawn(async move {
            loop {
                if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                match listener.accept() {
                    Ok((tcp_stream, peer_addr)) => {
                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::new(
                                Direction::Rx,
                                FrameLabel::ConnectionEvent,
                                format!("客户端连接: {}{}", peer_addr, if is_tls { " (TLS)" } else { "" }),
                            ));
                        }

                        // Wrap with TLS if configured
                        let slave_stream = if let Some(ref acceptor) = tls_acceptor {
                            // TLS handshake (blocking, done in spawn_blocking)
                            let acceptor = acceptor.clone();
                            let lc = log_collector.clone();
                            match acceptor.accept(tcp_stream) {
                                Ok(tls_stream) => {
                                    if let Some(ref lc) = lc {
                                        lc.try_add(LogEntry::new(
                                            Direction::Rx,
                                            FrameLabel::ConnectionEvent,
                                            format!("TLS 握手成功: {}", peer_addr),
                                        ));
                                    }
                                    SlaveStream::Tls(tls_stream)
                                }
                                Err(e) => {
                                    if let Some(ref lc) = lc {
                                        lc.try_add(LogEntry::new(
                                            Direction::Rx,
                                            FrameLabel::ConnectionEvent,
                                            format!("TLS 握手失败: {} - {}", peer_addr, e),
                                        ));
                                    }
                                    continue;
                                }
                            }
                        } else {
                            SlaveStream::Plain(tcp_stream)
                        };

                        let stations = stations.clone();
                        let lc = log_collector.clone();
                        let flag = shutdown_flag.clone();
                        tokio::task::spawn_blocking(move || {
                            handle_client(slave_stream, stations, lc, flag);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    }
                    Err(_) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        self.server_handle = Some(handle);
        self.state = ServerState::Running;

        if let Some(ref lc) = self.log_collector {
            lc.try_add(LogEntry::new(
                Direction::Tx,
                FrameLabel::ConnectionEvent,
                format!("服务器启动: {}{}", addr, if is_tls { " (TLS)" } else { "" }),
            ));
        }

        Ok(())
    }

    /// Stop the server.
    pub async fn stop(&mut self) -> Result<(), SlaveError> {
        if self.state == ServerState::Stopped {
            return Err(SlaveError::NotRunning);
        }

        self.shutdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);

        if let Some(handle) = self.server_handle.take() {
            let _ = handle.await;
        }

        self.state = ServerState::Stopped;

        if let Some(ref lc) = self.log_collector {
            lc.try_add(LogEntry::new(
                Direction::Tx,
                FrameLabel::ConnectionEvent,
                "服务器停止".to_string(),
            ));
        }

        Ok(())
    }
}

/// Handle a single client connection using the IEC 104 protocol.
fn handle_client(
    mut stream: SlaveStream,
    stations: SharedStations,
    log_collector: Option<Arc<LogCollector>>,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
) {
    stream.set_nonblocking(false).ok();
    stream.set_read_timeout(Some(std::time::Duration::from_millis(500))).ok();

    let mut buf = [0u8; 512];

    loop {
        if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        let n = match stream.read(&mut buf) {
            Ok(0) => break, // Connection closed
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        };

        let data = &buf[..n];

        // Log raw received frame
        if let Some(ref lc) = log_collector {
            if let Ok(frame) = crate::frame::parse_apci(data) {
                let summary = crate::frame::format_frame_summary(&frame);
                lc.try_add(LogEntry::with_raw_bytes(
                    Direction::Rx,
                    FrameLabel::IFrame(summary.clone()),
                    summary,
                    data.to_vec(),
                ));
            }
        }

        // Parse and respond to IEC 104 frames
        if data.len() >= 6 && data[0] == 0x68 {
            let ctrl1 = data[2];

            if ctrl1 & 0x03 == 0x03 {
                // U-frame
                match ctrl1 {
                    0x07 => {
                        // STARTDT ACT -> respond with STARTDT CON
                        let response = [0x68, 0x04, 0x0B, 0x00, 0x00, 0x00];
                        let _ = stream.write_all(&response);
                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::with_raw_bytes(
                                Direction::Tx,
                                FrameLabel::UStartCon,
                                "STARTDT CON",
                                response.to_vec(),
                            ));
                        }
                    }
                    0x13 => {
                        // STOPDT ACT -> respond with STOPDT CON
                        let response = [0x68, 0x04, 0x23, 0x00, 0x00, 0x00];
                        let _ = stream.write_all(&response);
                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::with_raw_bytes(
                                Direction::Tx,
                                FrameLabel::UStopCon,
                                "STOPDT CON",
                                response.to_vec(),
                            ));
                        }
                    }
                    0x43 => {
                        // TESTFR ACT -> respond with TESTFR CON
                        let response = [0x68, 0x04, 0x83, 0x00, 0x00, 0x00];
                        let _ = stream.write_all(&response);
                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::with_raw_bytes(
                                Direction::Tx,
                                FrameLabel::UTestCon,
                                "TESTFR CON",
                                response.to_vec(),
                            ));
                        }
                    }
                    _ => {}
                }
            } else if ctrl1 & 0x01 == 0 && data.len() >= 12 {
                // I-frame with ASDU
                let asdu_type = data[6];
                let _num_objects = data[7];
                let cause = data[8];
                let ca = u16::from_le_bytes([data[10], data[11]]);

                match asdu_type {
                    100 => {
                        // General Interrogation Command (C_IC_NA_1)
                        // Send activation confirmation
                        let mut ack = data[..n].to_vec();
                        ack[8] = 7; // COT = ActivationCon
                        // Update sequence numbers
                        let _ = stream.write_all(&ack);

                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::new(
                                Direction::Tx,
                                FrameLabel::GeneralInterrogation,
                                format!("GI 激活确认 CA={}", ca),
                            ));
                        }

                        // Send data points for the requested CA
                        // Clone station data first (to release the async lock), then write directly.
                        let station_clone = {
                            let rt = tokio::runtime::Handle::try_current();
                            if let Ok(handle) = rt {
                                let stations = stations.clone();
                                handle.block_on(async {
                                    let stations_read = stations.read().await;
                                    stations_read.get(&ca).cloned()
                                })
                            } else {
                                None
                            }
                        };
                        if let Some(ref station) = station_clone {
                            send_interrogation_response(&mut stream, station, &log_collector);
                        }

                        // Send activation termination
                        let mut term = data[..n].to_vec();
                        term[8] = 10; // COT = ActivationTermination
                        let _ = stream.write_all(&term);

                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::new(
                                Direction::Tx,
                                FrameLabel::GeneralInterrogation,
                                format!("GI 激活终止 CA={}", ca),
                            ));
                        }
                    }
                    103 => {
                        // Clock Synchronization (C_CS_NA_1)
                        let mut ack = data[..n].to_vec();
                        ack[8] = 7; // COT = ActivationCon
                        let _ = stream.write_all(&ack);

                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::new(
                                Direction::Tx,
                                FrameLabel::ClockSync,
                                format!("时钟同步确认 CA={}", ca),
                            ));
                        }
                    }
                    45 => {
                        // Single Command (C_SC_NA_1)
                        if data.len() >= 15 {
                            let ioa = u32::from_le_bytes([data[12], data[13], data[14], 0]);
                            let sco = data[15];
                            let value = sco & 0x01 != 0;

                            // Update data point
                            let rt = tokio::runtime::Handle::try_current();
                            if let Ok(handle) = rt {
                                let stations = stations.clone();
                                let _ = handle.block_on(async {
                                    let mut stations_w = stations.write().await;
                                    if let Some(station) = stations_w.get_mut(&ca) {
                                        if let Some(dp) = station.data_points.get_mut(ioa) {
                                            dp.value = DataPointValue::SinglePoint { value };
                                            dp.timestamp = Some(chrono::Utc::now());
                                        }
                                    }
                                });
                            }

                            // Send activation confirmation
                            let mut ack = data[..n].to_vec();
                            ack[8] = 7; // COT = ActivationCon
                            let _ = stream.write_all(&ack);

                            if let Some(ref lc) = log_collector {
                                lc.try_add(LogEntry::new(
                                    Direction::Tx,
                                    FrameLabel::SingleCommand,
                                    format!("单点命令确认 IOA={} val={} CA={}", ioa, value, ca),
                                ));
                            }
                        }
                    }
                    46 => {
                        // Double Command (C_DC_NA_1)
                        if data.len() >= 15 {
                            let ioa = u32::from_le_bytes([data[12], data[13], data[14], 0]);
                            let dco = data[15];
                            let value = dco & 0x03;

                            let rt = tokio::runtime::Handle::try_current();
                            if let Ok(handle) = rt {
                                let stations = stations.clone();
                                let _ = handle.block_on(async {
                                    let mut stations_w = stations.write().await;
                                    if let Some(station) = stations_w.get_mut(&ca) {
                                        if let Some(dp) = station.data_points.get_mut(ioa) {
                                            dp.value = DataPointValue::DoublePoint { value };
                                            dp.timestamp = Some(chrono::Utc::now());
                                        }
                                    }
                                });
                            }

                            let mut ack = data[..n].to_vec();
                            ack[8] = 7;
                            let _ = stream.write_all(&ack);

                            if let Some(ref lc) = log_collector {
                                lc.try_add(LogEntry::new(
                                    Direction::Tx,
                                    FrameLabel::DoubleCommand,
                                    format!("双点命令确认 IOA={} val={} CA={}", ioa, value, ca),
                                ));
                            }
                        }
                    }
                    50 => {
                        // Set-point, short float (C_SE_NC_1)
                        if data.len() >= 19 {
                            let ioa = u32::from_le_bytes([data[12], data[13], data[14], 0]);
                            let value = f32::from_le_bytes([data[15], data[16], data[17], data[18]]);

                            let rt = tokio::runtime::Handle::try_current();
                            if let Ok(handle) = rt {
                                let stations = stations.clone();
                                let _ = handle.block_on(async {
                                    let mut stations_w = stations.write().await;
                                    if let Some(station) = stations_w.get_mut(&ca) {
                                        if let Some(dp) = station.data_points.get_mut(ioa) {
                                            dp.value = DataPointValue::ShortFloat { value };
                                            dp.timestamp = Some(chrono::Utc::now());
                                        }
                                    }
                                });
                            }

                            let mut ack = data[..n].to_vec();
                            ack[8] = 7;
                            let _ = stream.write_all(&ack);

                            if let Some(ref lc) = log_collector {
                                lc.try_add(LogEntry::new(
                                    Direction::Tx,
                                    FrameLabel::SetpointFloat,
                                    format!("浮点设定值确认 IOA={} val={:.3} CA={}", ioa, value, ca),
                                ));
                            }
                        }
                    }
                    _ => {
                        // Unknown ASDU type - log but ignore
                        if let Some(ref lc) = log_collector {
                            lc.try_add(LogEntry::new(
                                Direction::Rx,
                                FrameLabel::IFrame(format!("Type{}", asdu_type)),
                                format!("未知 ASDU 类型={} CA={} COT={}", asdu_type, ca, cause),
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Send all data points for a station in response to general interrogation.
fn send_interrogation_response(
    stream: &mut SlaveStream,
    station: &Station,
    log_collector: &Option<Arc<LogCollector>>,
) {
    let ca_bytes = station.common_address.to_le_bytes();

    for point in station.data_points.all_sorted() {
        let ioa_bytes = point.ioa.to_le_bytes();

        let asdu = match &point.value {
            DataPointValue::SinglePoint { value } => {
                // M_SP_NA_1 (Type 1)
                let siq = if *value { 0x01 } else { 0x00 };
                build_i_frame(1, 20, &ca_bytes, &ioa_bytes[..3], &[siq])
            }
            DataPointValue::DoublePoint { value } => {
                // M_DP_NA_1 (Type 3)
                let diq = *value & 0x03;
                build_i_frame(3, 20, &ca_bytes, &ioa_bytes[..3], &[diq])
            }
            DataPointValue::Normalized { value } => {
                // M_ME_NA_1 (Type 9)
                let nva = (*value * 32767.0) as i16;
                let bytes = nva.to_le_bytes();
                let qds = 0u8; // good quality
                build_i_frame(9, 20, &ca_bytes, &ioa_bytes[..3], &[bytes[0], bytes[1], qds])
            }
            DataPointValue::Scaled { value } => {
                // M_ME_NB_1 (Type 11)
                let bytes = value.to_le_bytes();
                let qds = 0u8;
                build_i_frame(11, 20, &ca_bytes, &ioa_bytes[..3], &[bytes[0], bytes[1], qds])
            }
            DataPointValue::ShortFloat { value } => {
                // M_ME_NC_1 (Type 13)
                let bytes = value.to_le_bytes();
                let qds = 0u8;
                build_i_frame(13, 20, &ca_bytes, &ioa_bytes[..3], &[bytes[0], bytes[1], bytes[2], bytes[3], qds])
            }
            DataPointValue::IntegratedTotal { value, carry, sequence } => {
                // M_IT_NA_1 (Type 15)
                let bytes = value.to_le_bytes();
                let mut bcr = *sequence & 0x1F;
                if *carry { bcr |= 0x20; }
                build_i_frame(15, 20, &ca_bytes, &ioa_bytes[..3], &[bytes[0], bytes[1], bytes[2], bytes[3], bcr])
            }
            _ => continue,
        };

        let _ = stream.write_all(&asdu);
    }

    if let Some(ref lc) = log_collector {
        lc.try_add(LogEntry::new(
            Direction::Tx,
            FrameLabel::GeneralInterrogation,
            format!("GI 数据发送完成 CA={} 共{}点", station.common_address, station.data_points.len()),
        ));
    }
}

/// Build a minimal I-frame APDU.
fn build_i_frame(
    asdu_type: u8,
    cause: u8,
    ca: &[u8],
    ioa: &[u8],
    value_bytes: &[u8],
) -> Vec<u8> {
    // APCI header (6 bytes) + ASDU
    let asdu_len = 6 + ioa.len() + value_bytes.len(); // type(1) + num(1) + cause(2) + ca(2) + ioa + value
    let total_len = 4 + asdu_len; // control fields (4) + asdu

    let mut frame = Vec::with_capacity(2 + total_len);
    frame.push(0x68); // Start byte
    frame.push(total_len as u8); // Length

    // Control fields (I-frame, seq=0 for simplicity)
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

    // ASDU header
    frame.push(asdu_type); // Type ID
    frame.push(0x01);      // Number of objects = 1
    frame.push(cause);     // Cause of transmission
    frame.push(0x00);      // Originator address
    frame.extend_from_slice(&ca[..2]); // Common address

    // Information object
    frame.extend_from_slice(ioa);
    frame.extend_from_slice(value_bytes);

    frame
}

#[derive(Debug, thiserror::Error)]
pub enum SlaveError {
    #[error("IOA {0} already exists")]
    DuplicateIoa(u32),
    #[error("IOA {0} not found")]
    IoaNotFound(u32),
    #[error("station CA={0} already exists")]
    DuplicateStation(u16),
    #[error("station CA={0} not found")]
    StationNotFound(u16),
    #[error("server is already running")]
    AlreadyRunning,
    #[error("server is not running")]
    NotRunning,
    #[error("bind error: {0}")]
    BindError(String),
    #[error("TLS error: {0}")]
    TlsError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_station_creation() {
        let station = Station::new(1, "测试站");
        assert_eq!(station.common_address, 1);
        assert_eq!(station.name, "测试站");
        assert!(station.data_points.is_empty());
    }

    #[test]
    fn test_station_with_default_points() {
        let station = Station::with_default_points(1, "站1", 10);
        // 6 categories x 10 = 60 points
        assert_eq!(station.data_points.len(), 60);
        assert_eq!(station.object_defs.len(), 60);
    }

    #[test]
    fn test_station_add_remove_point() {
        let mut station = Station::new(1, "测试");
        let def = InformationObjectDef {
            ioa: 100,
            asdu_type: AsduTypeId::MSpNa1,
            category: DataCategory::SinglePoint,
            name: "测试点".to_string(),
            comment: String::new(),
        };

        station.add_point(def.clone()).unwrap();
        assert_eq!(station.data_points.len(), 1);

        // Duplicate should fail
        assert!(station.add_point(def).is_err());

        // Remove
        station.remove_point(100).unwrap();
        assert!(station.data_points.is_empty());

        // Remove again should fail
        assert!(station.remove_point(100).is_err());
    }

    #[tokio::test]
    async fn test_slave_server_station_management() {
        let server = SlaveServer::new(SlaveTransportConfig::default());
        let station = Station::new(1, "站1");
        server.add_station(station).await.unwrap();

        // Duplicate should fail
        assert!(server.add_station(Station::new(1, "重复")).await.is_err());

        // Remove
        let removed = server.remove_station(1).await.unwrap();
        assert_eq!(removed.common_address, 1);

        // Remove again should fail
        assert!(server.remove_station(1).await.is_err());
    }
}
