use crate::data_point::{DataPoint, DataPointMap, DataPointValue};
use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FrameLabel, LogEntry};
use crate::types::AsduTypeId;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// TLS Configuration
// ---------------------------------------------------------------------------

/// TLS configuration for a master connection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS
    pub enabled: bool,
    /// Path to CA certificate file (PEM format) for server verification
    #[serde(default)]
    pub ca_file: String,
    /// Path to client certificate file (PEM format) for mutual TLS
    #[serde(default)]
    pub cert_file: String,
    /// Path to client private key file (PEM format)
    #[serde(default)]
    pub key_file: String,
    /// Accept invalid/self-signed certificates (for testing)
    #[serde(default)]
    pub accept_invalid_certs: bool,
}

// ---------------------------------------------------------------------------
// Stream Abstraction
// ---------------------------------------------------------------------------

/// A stream that can be either plain TCP or TLS-wrapped.
enum MasterStream {
    Plain(TcpStream),
    Tls(native_tls::TlsStream<TcpStream>),
}

impl MasterStream {
    #[allow(dead_code)]
    fn try_clone(&self) -> std::io::Result<Self> {
        match self {
            MasterStream::Plain(s) => Ok(MasterStream::Plain(s.try_clone()?)),
            MasterStream::Tls(_) => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "TLS stream cannot be cloned",
                ))
            }
        }
    }
}

impl Read for MasterStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            MasterStream::Plain(s) => s.read(buf),
            MasterStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for MasterStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MasterStream::Plain(s) => s.write(buf),
            MasterStream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MasterStream::Plain(s) => s.flush(),
            MasterStream::Tls(s) => s.flush(),
        }
    }
}

// Implement Read/Write for &MasterStream (needed for shared access via RwLock)
impl Read for &MasterStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            MasterStream::Plain(s) => (&*s).read(buf),
            MasterStream::Tls(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Cannot read from shared TLS ref; use mutable access",
            )),
        }
    }
}

impl Write for &MasterStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MasterStream::Plain(s) => (&*s).write(buf),
            MasterStream::Tls(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Cannot write to shared TLS ref; use mutable access",
            )),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MasterStream::Plain(s) => (&*s).flush(),
            MasterStream::Tls(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Cannot flush shared TLS ref",
            )),
        }
    }
}

// We need Send + Sync for Arc<RwLock<..>>
// native_tls::TlsStream<TcpStream> is Send but not Sync by default.
// Since we guard with RwLock and only access mutably, this is safe.
unsafe impl Sync for MasterStream {}

// ---------------------------------------------------------------------------
// Master State & Config
// ---------------------------------------------------------------------------

/// Running state of a master connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Configuration for a master connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConfig {
    pub target_address: String,
    pub port: u16,
    pub common_address: u16,
    pub timeout_ms: u64,
    /// TLS configuration (optional)
    #[serde(default)]
    pub tls: TlsConfig,
}

impl Default for MasterConfig {
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

/// Received data storage.
pub type SharedReceivedData = Arc<RwLock<DataPointMap>>;

/// An IEC 104 master connection.
pub struct MasterConnection {
    pub config: MasterConfig,
    pub received_data: SharedReceivedData,
    pub log_collector: Option<Arc<LogCollector>>,
    state: Arc<RwLock<MasterState>>,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    stream: Arc<RwLock<Option<MasterStream>>>,
    /// Mutex-protected TLS stream for send operations (TLS streams cannot be cloned).
    tls_stream_mutex: Option<Arc<std::sync::Mutex<MasterStream>>>,
    receiver_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MasterConnection {
    pub fn new(config: MasterConfig) -> Self {
        Self {
            config,
            received_data: Arc::new(RwLock::new(DataPointMap::new())),
            log_collector: None,
            state: Arc::new(RwLock::new(MasterState::Disconnected)),
            shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stream: Arc::new(RwLock::new(None)),
            tls_stream_mutex: None,
            receiver_handle: None,
        }
    }

    pub fn with_log_collector(mut self, collector: Arc<LogCollector>) -> Self {
        self.log_collector = Some(collector);
        self
    }

    pub async fn state(&self) -> MasterState {
        *self.state.read().await
    }

    /// Connect to the remote IEC 104 slave (with optional TLS).
    pub async fn connect(&mut self) -> Result<(), MasterError> {
        if *self.state.read().await == MasterState::Connected {
            return Err(MasterError::AlreadyConnected);
        }

        *self.state.write().await = MasterState::Connecting;

        let addr = format!("{}:{}", self.config.target_address, self.config.port);
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);

        let tcp_stream = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| MasterError::ConnectionError(format!("Invalid address: {}", e)))?,
            timeout,
        ).map_err(|e| {
            *self.state.try_write().unwrap() = MasterState::Error;
            MasterError::ConnectionError(format!("Failed to connect to {}: {}", addr, e))
        })?;

        tcp_stream.set_read_timeout(Some(std::time::Duration::from_millis(500))).ok();
        tcp_stream.set_nodelay(true).ok();

        // Wrap with TLS if configured
        let master_stream = if self.config.tls.enabled {
            if let Some(ref lc) = self.log_collector {
                lc.try_add(LogEntry::new(
                    Direction::Tx,
                    FrameLabel::ConnectionEvent,
                    format!("TLS 握手中... {}", addr),
                ));
            }

            let tls_stream = self.create_tls_stream(tcp_stream)?;

            if let Some(ref lc) = self.log_collector {
                lc.try_add(LogEntry::new(
                    Direction::Rx,
                    FrameLabel::ConnectionEvent,
                    "TLS 握手成功".to_string(),
                ));
            }

            MasterStream::Tls(tls_stream)
        } else {
            MasterStream::Plain(tcp_stream)
        };

        // Send STARTDT ACT
        let startdt_act = [0x68, 0x04, 0x07, 0x00, 0x00, 0x00];
        // We need mutable access for TLS streams
        {
            match &master_stream {
                MasterStream::Plain(s) => {
                    (&*s).write_all(&startdt_act)
                        .map_err(|e| MasterError::ConnectionError(format!("Failed to send STARTDT: {}", e)))?;
                }
                MasterStream::Tls(_) => {
                    // For TLS, we'll write after storing the stream
                }
            }
        }

        if let Some(ref lc) = self.log_collector {
            lc.try_add(LogEntry::with_raw_bytes(
                Direction::Tx,
                FrameLabel::UStartAct,
                format!("STARTDT ACT -> {}{}", addr, if self.config.tls.enabled { " (TLS)" } else { "" }),
                startdt_act.to_vec(),
            ));
        }

        // For TLS streams, we can't clone, so we use a different approach:
        // Store the stream in a mutex and share it between sender and receiver.
        let is_tls = self.config.tls.enabled;

        if is_tls {
            // For TLS: use Arc<Mutex> for shared mutable access
            let stream_mutex = Arc::new(std::sync::Mutex::new(master_stream));

            // Write STARTDT ACT through the mutex
            {
                let mut locked = stream_mutex.lock().unwrap();
                locked.write_all(&startdt_act)
                    .map_err(|e| MasterError::ConnectionError(format!("Failed to send STARTDT: {}", e)))?;
            }

            *self.state.write().await = MasterState::Connected;

            // Start receiver thread with mutex-based stream access
            self.shutdown_flag.store(false, std::sync::atomic::Ordering::SeqCst);
            let shutdown_flag = self.shutdown_flag.clone();
            let received_data = self.received_data.clone();
            let log_collector = self.log_collector.clone();
            let state = self.state.clone();
            let stream_for_receiver = stream_mutex.clone();

            let handle = tokio::task::spawn_blocking(move || {
                receive_loop_mutex(stream_for_receiver, received_data, log_collector, shutdown_flag, state);
            });

            self.receiver_handle = Some(handle);

            // Store the mutex for send/disconnect operations
            *self.stream.write().await = None;
            self.tls_stream_mutex = Some(stream_mutex);
        } else {
            // For plain TCP: clone the stream for the receiver thread
            let stream_clone = match &master_stream {
                MasterStream::Plain(s) => s.try_clone()
                    .map_err(|e| MasterError::ConnectionError(format!("Failed to clone stream: {}", e)))?,
                _ => unreachable!(),
            };

            *self.stream.write().await = Some(master_stream);
            *self.state.write().await = MasterState::Connected;

            self.shutdown_flag.store(false, std::sync::atomic::Ordering::SeqCst);
            let shutdown_flag = self.shutdown_flag.clone();
            let received_data = self.received_data.clone();
            let log_collector = self.log_collector.clone();
            let state = self.state.clone();

            let handle = tokio::task::spawn_blocking(move || {
                receive_loop(stream_clone, received_data, log_collector, shutdown_flag, state);
            });

            self.receiver_handle = Some(handle);
        }

        if let Some(ref lc) = self.log_collector {
            lc.try_add(LogEntry::new(
                Direction::Rx,
                FrameLabel::ConnectionEvent,
                format!("已连接到 {}{}", addr, if is_tls { " (TLS)" } else { "" }),
            ));
        }

        Ok(())
    }

    /// Create a TLS stream from a TCP stream using the configured certificates.
    fn create_tls_stream(&self, tcp_stream: TcpStream) -> Result<native_tls::TlsStream<TcpStream>, MasterError> {
        let mut builder = native_tls::TlsConnector::builder();

        // Set minimum TLS version to 1.2 (IEC 62351 requirement)
        builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));

        // Load CA certificate if provided
        if !self.config.tls.ca_file.is_empty() {
            let ca_pem = std::fs::read(&self.config.tls.ca_file)
                .map_err(|e| MasterError::TlsError(format!("读取 CA 证书失败 {}: {}", self.config.tls.ca_file, e)))?;
            let ca_cert = native_tls::Certificate::from_pem(&ca_pem)
                .map_err(|e| MasterError::TlsError(format!("解析 CA 证书失败: {}", e)))?;
            builder.add_root_certificate(ca_cert);
        }

        // Load client certificate and key if provided (mutual TLS)
        if !self.config.tls.cert_file.is_empty() && !self.config.tls.key_file.is_empty() {
            let cert_pem = std::fs::read(&self.config.tls.cert_file)
                .map_err(|e| MasterError::TlsError(format!("读取客户端证书失败 {}: {}", self.config.tls.cert_file, e)))?;
            let key_pem = std::fs::read(&self.config.tls.key_file)
                .map_err(|e| MasterError::TlsError(format!("读取客户端密钥失败 {}: {}", self.config.tls.key_file, e)))?;

            let identity = native_tls::Identity::from_pkcs8(&cert_pem, &key_pem)
                .map_err(|e| MasterError::TlsError(format!("加载客户端身份失败: {}", e)))?;
            builder.identity(identity);
        }

        // Accept invalid certs (for self-signed testing)
        if self.config.tls.accept_invalid_certs {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }

        let connector = builder.build()
            .map_err(|e| MasterError::TlsError(format!("创建 TLS 连接器失败: {}", e)))?;

        let domain = &self.config.target_address;
        let tls_stream = connector.connect(domain, tcp_stream)
            .map_err(|e| MasterError::TlsError(format!("TLS 握手失败: {}", e)))?;

        Ok(tls_stream)
    }

    /// Disconnect from the remote slave.
    pub async fn disconnect(&mut self) -> Result<(), MasterError> {
        if *self.state.read().await == MasterState::Disconnected {
            return Err(MasterError::NotConnected);
        }

        // Send STOPDT ACT (best effort)
        let stopdt = [0x68, 0x04, 0x13, 0x00, 0x00, 0x00];
        if let Some(ref mutex) = self.tls_stream_mutex {
            // TLS path
            if let Ok(mut stream) = mutex.lock() {
                let _ = stream.write_all(&stopdt);
            }
        } else {
            // Plain TCP path
            let stream_guard = self.stream.read().await;
            if let Some(ref stream) = *stream_guard {
                match stream {
                    MasterStream::Plain(s) => { let _ = (&*s).write_all(&stopdt); }
                    MasterStream::Tls(_) => {}
                }
            }
        }

        self.shutdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);

        if let Some(handle) = self.receiver_handle.take() {
            let _ = handle.await;
        }

        *self.stream.write().await = None;
        self.tls_stream_mutex = None;
        *self.state.write().await = MasterState::Disconnected;

        if let Some(ref lc) = self.log_collector {
            lc.try_add(LogEntry::new(
                Direction::Tx,
                FrameLabel::ConnectionEvent,
                "已断开连接".to_string(),
            ));
        }

        Ok(())
    }

    /// Send General Interrogation command.
    pub async fn send_interrogation(&self, ca: u16) -> Result<(), MasterError> {
        let frame = build_gi_command(ca);
        self.send_frame(&frame, "GI", FrameLabel::GeneralInterrogation, ca).await
    }

    /// Send Clock Synchronization command.
    pub async fn send_clock_sync(&self, ca: u16) -> Result<(), MasterError> {
        let frame = build_clock_sync_command(ca);
        self.send_frame(&frame, "时钟同步", FrameLabel::ClockSync, ca).await
    }

    /// Send Counter Interrogation command.
    pub async fn send_counter_read(&self, ca: u16) -> Result<(), MasterError> {
        let frame = build_counter_read_command(ca);
        self.send_frame(&frame, "累计量召唤", FrameLabel::CounterRead, ca).await
    }

    /// Send Single Command.
    pub async fn send_single_command(&self, ioa: u32, value: bool, select: bool, ca: u16) -> Result<(), MasterError> {
        let frame = build_single_command(ca, ioa, value, select);
        let detail = format!("单点命令 IOA={} val={} sel={}", ioa, value, select);
        self.send_frame(&frame, &detail, FrameLabel::SingleCommand, ca).await
    }

    /// Send Double Command.
    pub async fn send_double_command(&self, ioa: u32, value: u8, select: bool, ca: u16) -> Result<(), MasterError> {
        let frame = build_double_command(ca, ioa, value, select);
        let detail = format!("双点命令 IOA={} val={} sel={}", ioa, value, select);
        self.send_frame(&frame, &detail, FrameLabel::DoubleCommand, ca).await
    }

    /// Send Set-point (short float) command.
    pub async fn send_setpoint_float(&self, ioa: u32, value: f32, ca: u16) -> Result<(), MasterError> {
        let frame = build_setpoint_float_command(ca, ioa, value);
        let detail = format!("浮点设定值 IOA={} val={:.3}", ioa, value);
        self.send_frame(&frame, &detail, FrameLabel::SetpointFloat, ca).await
    }

    async fn send_frame(&self, frame: &[u8], detail: &str, label: FrameLabel, ca: u16) -> Result<(), MasterError> {
        if let Some(ref mutex) = self.tls_stream_mutex {
            // TLS path: write through the shared mutex
            let mut stream = mutex.lock()
                .map_err(|e| MasterError::SendError(format!("mutex lock failed: {}", e)))?;
            stream.write_all(frame)
                .map_err(|e| MasterError::SendError(format!("{}: {}", detail, e)))?;
        } else {
            // Plain TCP path
            let stream_guard = self.stream.read().await;
            let stream = stream_guard.as_ref()
                .ok_or(MasterError::NotConnected)?;
            match stream {
                MasterStream::Plain(s) => {
                    (&*s).write_all(frame)
                        .map_err(|e| MasterError::SendError(format!("{}: {}", detail, e)))?;
                }
                MasterStream::Tls(_) => unreachable!("TLS stream should use tls_stream_mutex"),
            }
        }

        if let Some(ref lc) = self.log_collector {
            lc.try_add(LogEntry::with_raw_bytes(
                Direction::Tx,
                label,
                format!("{} CA={}", detail, ca),
                frame.to_vec(),
            ));
        }

        Ok(())
    }
}

/// Background receive loop for plain TCP connections.
fn receive_loop(
    mut stream: TcpStream,
    received_data: SharedReceivedData,
    log_collector: Option<Arc<LogCollector>>,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    state: Arc<RwLock<MasterState>>,
) {
    let mut buf = [0u8; 1024];

    loop {
        if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        let n = match stream.read(&mut buf) {
            Ok(0) => {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let state = state.clone();
                    handle.block_on(async { *state.write().await = MasterState::Disconnected; });
                }
                if let Some(ref lc) = log_collector {
                    lc.try_add(LogEntry::new(Direction::Rx, FrameLabel::ConnectionEvent, "连接已关闭"));
                }
                break;
            }
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        };

        let data = &buf[..n];
        let mut offset = 0;
        while offset < n {
            if data[offset] != 0x68 || offset + 1 >= n {
                offset += 1;
                continue;
            }
            let frame_len = data[offset + 1] as usize + 2;
            if offset + frame_len > n { break; }
            let frame_data = &data[offset..offset + frame_len];
            process_received_frame(frame_data, &received_data, &log_collector, &mut stream);
            offset += frame_len;
        }
    }
}

/// Background receive loop for TLS connections using a shared Mutex.
fn receive_loop_mutex(
    stream: Arc<std::sync::Mutex<MasterStream>>,
    received_data: SharedReceivedData,
    log_collector: Option<Arc<LogCollector>>,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    state: Arc<RwLock<MasterState>>,
) {
    let mut buf = [0u8; 1024];

    loop {
        if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        let n = {
            let mut locked = match stream.lock() {
                Ok(s) => s,
                Err(_) => break,
            };
            match locked.read(&mut buf) {
                Ok(0) => {
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        let state = state.clone();
                        handle.block_on(async { *state.write().await = MasterState::Disconnected; });
                    }
                    if let Some(ref lc) = log_collector {
                        lc.try_add(LogEntry::new(Direction::Rx, FrameLabel::ConnectionEvent, "连接已关闭"));
                    }
                    break;
                }
                Ok(n) => n,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(_) => break,
            }
        };

        let data = &buf[..n];
        let mut offset = 0;
        while offset < n {
            if data[offset] != 0x68 || offset + 1 >= n {
                offset += 1;
                continue;
            }
            let frame_len = data[offset + 1] as usize + 2;
            if offset + frame_len > n { break; }
            let frame_data = &data[offset..offset + frame_len];
            process_received_frame_mutex(frame_data, &received_data, &log_collector, &stream);
            offset += frame_len;
        }
    }
}

/// Process a single received IEC 104 frame (plain TCP version).
fn process_received_frame(
    data: &[u8],
    received_data: &SharedReceivedData,
    log_collector: &Option<Arc<LogCollector>>,
    stream: &mut TcpStream,
) {
    if data.len() < 6 { return; }
    let ctrl1 = data[2];

    if ctrl1 & 0x03 == 0x03 {
        log_frame(data, log_collector);
        if ctrl1 == 0x43 {
            let response = [0x68, 0x04, 0x83, 0x00, 0x00, 0x00];
            let _ = stream.write_all(&response);
        }
    } else if ctrl1 & 0x01 == 0 && data.len() >= 12 {
        parse_and_store_asdu(data, received_data, log_collector);
        let s_frame = [0x68, 0x04, 0x01, 0x00, 0x00, 0x00];
        let _ = stream.write_all(&s_frame);
    }
}

/// Process a single received IEC 104 frame (TLS/Mutex version).
fn process_received_frame_mutex(
    data: &[u8],
    received_data: &SharedReceivedData,
    log_collector: &Option<Arc<LogCollector>>,
    stream: &Arc<std::sync::Mutex<MasterStream>>,
) {
    if data.len() < 6 { return; }
    let ctrl1 = data[2];

    if ctrl1 & 0x03 == 0x03 {
        log_frame(data, log_collector);
        if ctrl1 == 0x43 {
            let response = [0x68, 0x04, 0x83, 0x00, 0x00, 0x00];
            if let Ok(mut locked) = stream.lock() {
                let _ = locked.write_all(&response);
            }
        }
    } else if ctrl1 & 0x01 == 0 && data.len() >= 12 {
        parse_and_store_asdu(data, received_data, log_collector);
        let s_frame = [0x68, 0x04, 0x01, 0x00, 0x00, 0x00];
        if let Ok(mut locked) = stream.lock() {
            let _ = locked.write_all(&s_frame);
        }
    }
}

/// Log a received U-frame.
fn log_frame(data: &[u8], log_collector: &Option<Arc<LogCollector>>) {
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
}

/// Parse ASDU from an I-frame and store data points.
fn parse_and_store_asdu(
    data: &[u8],
    received_data: &SharedReceivedData,
    log_collector: &Option<Arc<LogCollector>>,
) {
    let asdu_type = data[6];
    let num_objects = data[7] & 0x7F;
    let cause = data[8];
    let ca = u16::from_le_bytes([data[10], data[11]]);

    if let Some(ref lc) = log_collector {
        let type_name = AsduTypeId::from_u8(asdu_type)
            .map(|t| t.name().to_string())
            .unwrap_or_else(|| format!("Type{}", asdu_type));
        lc.try_add(LogEntry::with_raw_bytes(
            Direction::Rx,
            FrameLabel::IFrame(type_name.clone()),
            format!("{} CA={} n={} COT={}", type_name, ca, num_objects, cause),
            data.to_vec(),
        ));
    }

    let mut obj_offset = 12;
    for _ in 0..num_objects {
        if obj_offset + 3 > data.len() { break; }
        let ioa = u32::from_le_bytes([data[obj_offset], data[obj_offset + 1], data[obj_offset + 2], 0]);
        obj_offset += 3;

        let (value, bytes_consumed) = match asdu_type {
            1 | 30 => {
                if obj_offset >= data.len() { break; }
                let siq = data[obj_offset];
                let val = DataPointValue::SinglePoint { value: siq & 0x01 != 0 };
                (val, 1 + if asdu_type == 30 { 7 } else { 0 })
            }
            3 | 31 => {
                if obj_offset >= data.len() { break; }
                let diq = data[obj_offset];
                let val = DataPointValue::DoublePoint { value: diq & 0x03 };
                (val, 1 + if asdu_type == 31 { 7 } else { 0 })
            }
            9 | 34 => {
                if obj_offset + 2 >= data.len() { break; }
                let nva = i16::from_le_bytes([data[obj_offset], data[obj_offset + 1]]);
                let val = DataPointValue::Normalized { value: nva as f32 / 32767.0 };
                (val, 3 + if asdu_type == 34 { 7 } else { 0 })
            }
            11 | 35 => {
                if obj_offset + 2 >= data.len() { break; }
                let sva = i16::from_le_bytes([data[obj_offset], data[obj_offset + 1]]);
                let val = DataPointValue::Scaled { value: sva };
                (val, 3 + if asdu_type == 35 { 7 } else { 0 })
            }
            13 | 36 => {
                if obj_offset + 4 >= data.len() { break; }
                let fval = f32::from_le_bytes([
                    data[obj_offset], data[obj_offset + 1],
                    data[obj_offset + 2], data[obj_offset + 3],
                ]);
                let val = DataPointValue::ShortFloat { value: fval };
                (val, 5 + if asdu_type == 36 { 7 } else { 0 })
            }
            15 | 37 => {
                if obj_offset + 4 >= data.len() { break; }
                let counter = i32::from_le_bytes([
                    data[obj_offset], data[obj_offset + 1],
                    data[obj_offset + 2], data[obj_offset + 3],
                ]);
                let bcr = if obj_offset + 4 < data.len() { data[obj_offset + 4] } else { 0 };
                let carry = bcr & 0x20 != 0;
                let sequence = bcr & 0x1F;
                let val = DataPointValue::IntegratedTotal { value: counter, carry, sequence };
                (val, 5 + if asdu_type == 37 { 7 } else { 0 })
            }
            _ => { break; }
        };

        obj_offset += bytes_consumed;

        let asdu_id = AsduTypeId::from_u8(asdu_type).unwrap_or(AsduTypeId::MSpNa1);
        let point = DataPoint::with_value(ioa, asdu_id, value);

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let rd = received_data.clone();
            let _ = handle.block_on(async { rd.write().await.insert(point); });
        }
    }
}

// --- Command frame builders ---

fn build_gi_command(ca: u16) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    vec![
        0x68, 0x0E,
        0x00, 0x00, 0x00, 0x00,
        100, 0x01, 6, 0x00,
        ca_bytes[0], ca_bytes[1],
        0x00, 0x00, 0x00,
        0x14,
    ]
}

fn build_clock_sync_command(ca: u16) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let now = chrono::Utc::now();
    let ms = (now.timestamp_subsec_millis() as u16) + ((now.format("%S").to_string().parse::<u16>().unwrap_or(0)) * 1000);
    let min = now.format("%M").to_string().parse::<u8>().unwrap_or(0);
    let hour = now.format("%H").to_string().parse::<u8>().unwrap_or(0);
    let day = now.format("%d").to_string().parse::<u8>().unwrap_or(1);
    let month = now.format("%m").to_string().parse::<u8>().unwrap_or(1);
    let year = (now.format("%Y").to_string().parse::<u16>().unwrap_or(2024) % 100) as u8;
    let ms_bytes = ms.to_le_bytes();

    vec![
        0x68, 0x14,
        0x00, 0x00, 0x00, 0x00,
        103, 0x01, 6, 0x00,
        ca_bytes[0], ca_bytes[1],
        0x00, 0x00, 0x00,
        ms_bytes[0], ms_bytes[1],
        min, hour, day, month, year,
    ]
}

fn build_counter_read_command(ca: u16) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    vec![
        0x68, 0x0E,
        0x00, 0x00, 0x00, 0x00,
        101, 0x01, 6, 0x00,
        ca_bytes[0], ca_bytes[1],
        0x00, 0x00, 0x00,
        0x05,
    ]
}

fn build_single_command(ca: u16, ioa: u32, value: bool, select: bool) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let mut sco = if value { 0x01 } else { 0x00 };
    if select { sco |= 0x80; }
    vec![
        0x68, 0x0E,
        0x00, 0x00, 0x00, 0x00,
        45, 0x01, 6, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        sco,
    ]
}

fn build_double_command(ca: u16, ioa: u32, value: u8, select: bool) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let mut dco = value & 0x03;
    if select { dco |= 0x80; }
    vec![
        0x68, 0x0E,
        0x00, 0x00, 0x00, 0x00,
        46, 0x01, 6, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        dco,
    ]
}

fn build_setpoint_float_command(ca: u16, ioa: u32, value: f32) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let val_bytes = value.to_le_bytes();
    vec![
        0x68, 0x12,
        0x00, 0x00, 0x00, 0x00,
        50, 0x01, 6, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3],
        0x00,
    ]
}

#[derive(Debug, thiserror::Error)]
pub enum MasterError {
    #[error("already connected")]
    AlreadyConnected,
    #[error("not connected")]
    NotConnected,
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("send error: {0}")]
    SendError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_config_default() {
        let config = MasterConfig::default();
        assert_eq!(config.port, 2404);
        assert_eq!(config.common_address, 1);
        assert!(!config.tls.enabled);
    }

    #[test]
    fn test_tls_config_default() {
        let tls = TlsConfig::default();
        assert!(!tls.enabled);
        assert!(tls.ca_file.is_empty());
        assert!(tls.cert_file.is_empty());
        assert!(tls.key_file.is_empty());
        assert!(!tls.accept_invalid_certs);
    }

    #[test]
    fn test_build_gi_command() {
        let frame = build_gi_command(1);
        assert_eq!(frame[0], 0x68);
        assert_eq!(frame[6], 100);
        assert_eq!(frame[8], 6);
    }

    #[test]
    fn test_build_single_command() {
        let frame = build_single_command(1, 100, true, false);
        assert_eq!(frame[6], 45);
        assert_eq!(frame[12], 100);
        assert_eq!(frame[15], 0x01);
    }
}
