# TLS End-to-End Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add end-to-end tests that verify IEC 104 protocol functionality (GI, spontaneous, control) works correctly over TLS, with automated packet capture and encryption assertions.

**Architecture:** Single test file `tests/tls_e2e.rs` with two internal modules (`cert_gen` for rcgen-based certificate generation, `capture` for tcpdump/tshark packet capture and analysis). Four test functions cover one-way TLS handshake, mTLS handshake, and full protocol flows over both TLS modes.

**Tech Stack:** Rust, tokio test, rcgen (cert gen), tempfile (temp dirs), native-tls (existing), tcpdump (capture), tshark (pcap analysis)

**Spec:** `docs/superpowers/specs/2026-04-02-tls-e2e-testing-design.md`

---

## Important Notes

**macOS BPF permissions:** `tcpdump` on macOS requires BPF device access. Installing Wireshark (`brew install --cask wireshark`) automatically grants this. Without it, capture needs `sudo`. Tests skip gracefully if capture tools lack permissions.

**mTLS limitation:** The current slave TLS acceptor (`slave.rs:338-346`) does NOT enforce `require_client_cert` or load `ca_file` for client verification. The `native_tls::TlsAcceptorBuilder` lacks cross-platform client cert enforcement API. The mTLS tests verify that the master CAN present a client certificate and the connection works — but the server does not reject clients without certificates. This is a known gap in the production code, not a test issue.

---

### Task 1: Add Dependencies and Configuration

**Files:**
- Modify: `crates/iec104sim-core/Cargo.toml:18-19`
- Modify: `.gitignore:28`

- [ ] **Step 1: Add dev-dependencies to Cargo.toml**

In `crates/iec104sim-core/Cargo.toml`, add `rcgen` and `tempfile` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
rcgen = "0.13"
tempfile = "3"
```

- [ ] **Step 2: Add pcap directory to .gitignore**

Append to `.gitignore`:

```
# Packet captures from TLS tests
crates/iec104sim-core/tests/pcap/
```

- [ ] **Step 3: Create pcap output directory**

```bash
mkdir -p crates/iec104sim-core/tests/pcap
```

- [ ] **Step 4: Verify dependencies resolve**

Run: `cd "crates/iec104sim-core" && cargo check --tests 2>&1 | tail -5`
Expected: compilation succeeds (no errors)

- [ ] **Step 5: Commit**

```bash
git add crates/iec104sim-core/Cargo.toml .gitignore
git commit -m "chore: add rcgen and tempfile dev-dependencies for TLS tests"
```

---

### Task 2: Create Test File with Certificate Generation Module

**Files:**
- Create: `crates/iec104sim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Write the cert_gen module and a verification test**

Create `crates/iec104sim-core/tests/tls_e2e.rs` with the certificate generation module and a test that verifies certs are generated and written correctly:

```rust
use iec104sim_core::data_point::DataPointValue;
use iec104sim_core::master::{MasterConfig, MasterConnection, TlsConfig};
use iec104sim_core::slave::{SlaveServer, SlaveTlsConfig, SlaveTransportConfig, Station};
use iec104sim_core::types::AsduTypeId;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::time::{sleep, Duration};

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

// =========================================================================
// Tool availability check
// =========================================================================

fn check_tools_available() -> bool {
    let tcpdump_ok = Command::new("tcpdump").arg("--version").output().is_ok();
    let tshark_ok = Command::new("tshark").arg("--version").output().is_ok();
    if !tcpdump_ok {
        eprintln!("SKIP: tcpdump not found in PATH. Install with: brew install tcpdump (or use system tcpdump)");
    }
    if !tshark_ok {
        eprintln!("SKIP: tshark not found in PATH. Install with: brew install wireshark");
    }
    tcpdump_ok && tshark_ok
}

// =========================================================================
// Module: cert_gen — Dynamic certificate generation with rcgen
// =========================================================================

mod cert_gen {
    use std::path::{Path, PathBuf};

    pub struct TestCerts {
        pub ca_cert_pem: String,
        pub server_cert_pem: String,
        pub server_key_pem: String,
        pub client_cert_pem: String,
        pub client_key_pem: String,
    }

    pub struct CertPaths {
        pub ca_cert: PathBuf,
        pub server_cert: PathBuf,
        pub server_key: PathBuf,
        pub client_cert: PathBuf,
        pub client_key: PathBuf,
    }

    /// Generate a full certificate chain: CA -> Server + Client.
    pub fn generate() -> TestCerts {
        use rcgen::{
            CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, BasicConstraints,
            KeyUsagePurpose, SanType, KeyPair,
        };

        // --- CA certificate ---
        let mut ca_params = CertificateParams::new(vec!["IEC104 Test CA".to_string()]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];
        ca_params.distinguished_name.push(DnType::CommonName, "IEC104 Test CA");
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();

        // --- Server certificate ---
        let mut server_params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        server_params.subject_alt_names = vec![
            SanType::DnsName("localhost".try_into().unwrap()),
            SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
        ];
        server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        server_params.distinguished_name.push(DnType::CommonName, "IEC104 Test Server");
        let server_key = KeyPair::generate().unwrap();
        let server_cert = server_params.signed_by(&server_key, &ca_cert, &ca_key).unwrap();

        // --- Client certificate ---
        let mut client_params = CertificateParams::new(vec!["IEC104 Test Client".to_string()]).unwrap();
        client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        client_params.distinguished_name.push(DnType::CommonName, "IEC104 Test Client");
        let client_key = KeyPair::generate().unwrap();
        let client_cert = client_params.signed_by(&client_key, &ca_cert, &ca_key).unwrap();

        TestCerts {
            ca_cert_pem: ca_cert.pem(),
            server_cert_pem: server_cert.pem(),
            server_key_pem: server_key.serialize_pem(),
            client_cert_pem: client_cert.pem(),
            client_key_pem: client_key.serialize_pem(),
        }
    }

    /// Write all PEM files to the given directory, return paths.
    pub fn write_to_dir(certs: &TestCerts, dir: &Path) -> CertPaths {
        let paths = CertPaths {
            ca_cert: dir.join("ca.pem"),
            server_cert: dir.join("server.pem"),
            server_key: dir.join("server-key.pem"),
            client_cert: dir.join("client.pem"),
            client_key: dir.join("client-key.pem"),
        };
        std::fs::write(&paths.ca_cert, &certs.ca_cert_pem).unwrap();
        std::fs::write(&paths.server_cert, &certs.server_cert_pem).unwrap();
        std::fs::write(&paths.server_key, &certs.server_key_pem).unwrap();
        std::fs::write(&paths.client_cert, &certs.client_cert_pem).unwrap();
        std::fs::write(&paths.client_key, &certs.client_key_pem).unwrap();
        paths
    }
}

// =========================================================================
// Verification test: cert generation
// =========================================================================

#[test]
fn test_cert_generation() {
    let certs = cert_gen::generate();
    assert!(certs.ca_cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(certs.server_cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(certs.server_key_pem.contains("BEGIN PRIVATE KEY"));
    assert!(certs.client_cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(certs.client_key_pem.contains("BEGIN PRIVATE KEY"));

    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());
    assert!(paths.ca_cert.exists());
    assert!(paths.server_cert.exists());
    assert!(paths.server_key.exists());
    assert!(paths.client_cert.exists());
    assert!(paths.client_key.exists());
}
```

- [ ] **Step 2: Run the cert generation test**

Run: `cargo test --package iec104sim-core --test tls_e2e test_cert_generation -- --nocapture`
Expected: PASS — all PEM strings contain expected markers, all files exist.

- [ ] **Step 3: Commit**

```bash
git add crates/iec104sim-core/tests/tls_e2e.rs
git commit -m "test: add TLS cert generation module with rcgen"
```

---

### Task 3: Add Packet Capture Module

**Files:**
- Modify: `crates/iec104sim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Add the capture module after cert_gen**

Append the `capture` module and its helpers to `tls_e2e.rs`, after the `test_cert_generation` test:

```rust
// =========================================================================
// Module: capture — Packet capture with tcpdump + analysis with tshark
// =========================================================================

mod capture {
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};

    pub struct PacketCapture {
        child: Child,
        pub pcap_path: PathBuf,
    }

    /// Start tcpdump capturing on loopback for the given port.
    /// Returns a handle to stop capture later.
    pub fn start(test_name: &str, port: u16) -> Result<PacketCapture, String> {
        let pcap_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pcap");
        std::fs::create_dir_all(&pcap_dir).map_err(|e| format!("create pcap dir: {}", e))?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let pcap_path = pcap_dir.join(format!("{}_{}.pcap", test_name, timestamp));

        let child = Command::new("tcpdump")
            .args([
                "-i", "lo0",
                "-w", pcap_path.to_str().unwrap(),
                "-s", "0",           // capture full packets
                &format!("port {}", port),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn tcpdump: {} (do you have BPF permissions? Try: brew install --cask wireshark)", e))?;

        Ok(PacketCapture { child, pcap_path })
    }

    impl PacketCapture {
        /// Stop capturing. Sends SIGTERM and waits for tcpdump to flush.
        pub fn stop(&mut self) -> Result<(), String> {
            // Send SIGTERM
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGTERM);
            }
            self.child.wait().map_err(|e| format!("wait tcpdump: {}", e))?;
            // Brief delay to ensure file is fully written
            std::thread::sleep(std::time::Duration::from_millis(200));
            Ok(())
        }
    }

    /// Assert that the pcap contains a valid TLS session:
    /// 1. TLS handshake present (ClientHello + ServerHello)
    /// 2. No plaintext IEC 104 frames visible
    /// 3. Encrypted application data present
    pub fn assert_tls_encrypted(pcap_path: &Path, port: u16) {
        let pcap = pcap_path.to_str().unwrap();

        // 1. Check TLS handshake exists
        let output = Command::new("tshark")
            .args(["-r", pcap, "-Y", "tls.handshake", "-T", "fields", "-e", "tls.handshake.type"])
            .output()
            .expect("failed to run tshark");
        let handshake_types = String::from_utf8_lossy(&output.stdout);
        assert!(
            handshake_types.contains("1"),
            "No ClientHello found in pcap: {}\ntshark output: {}",
            pcap, handshake_types
        );
        assert!(
            handshake_types.contains("2"),
            "No ServerHello found in pcap: {}\ntshark output: {}",
            pcap, handshake_types
        );

        // 2. Check no plaintext IEC 104 is visible (TLS should hide it)
        let output = Command::new("tshark")
            .args(["-r", pcap, "-Y", "iec60870_104", "-T", "fields", "-e", "frame.number"])
            .output()
            .expect("failed to run tshark");
        let iec104_frames = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert!(
            iec104_frames.is_empty(),
            "Plaintext IEC 104 frames leaked through TLS! pcap: {}\nFrame numbers: {}",
            pcap, iec104_frames
        );

        // 3. Check encrypted application data exists
        let output = Command::new("tshark")
            .args([
                "-r", pcap,
                "-Y", &format!("tls.record.content_type == 23 && tcp.port == {}", port),
                "-T", "fields", "-e", "frame.number",
            ])
            .output()
            .expect("failed to run tshark");
        let app_data = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert!(
            !app_data.is_empty(),
            "No encrypted application data found in pcap: {}",
            pcap
        );

        eprintln!("  TLS assertions passed. pcap: {}", pcap);
    }
}
```

- [ ] **Step 2: Add libc dependency for SIGTERM**

In `crates/iec104sim-core/Cargo.toml`, add `libc` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
rcgen = "0.13"
tempfile = "3"
libc = "0.2"
chrono = "0.4"
```

- [ ] **Step 3: Add chrono import to the capture module**

The capture module uses `chrono::Utc::now()`. Add `chrono` as a dev-dependency (step 2 already includes it). The main crate already depends on chrono, but dev-dependencies for integration tests need it explicitly.

- [ ] **Step 4: Verify compilation**

Run: `cargo test --package iec104sim-core --test tls_e2e --no-run 2>&1 | tail -5`
Expected: compilation succeeds

- [ ] **Step 5: Commit**

```bash
git add crates/iec104sim-core/tests/tls_e2e.rs crates/iec104sim-core/Cargo.toml
git commit -m "test: add packet capture module with tcpdump/tshark"
```

---

### Task 4: Write test_tls_handshake_one_way

**Files:**
- Modify: `crates/iec104sim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Write the one-way TLS handshake test**

Append to `tls_e2e.rs`:

```rust
// =========================================================================
// Test: One-way TLS handshake (server auth only)
// =========================================================================
#[tokio::test]
async fn test_tls_handshake_one_way() {
    if !check_tools_available() { return; }

    let port = free_port();
    let certs = cert_gen::generate();
    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());

    // Start slave with TLS enabled, no client cert required
    let transport = SlaveTransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        tls: SlaveTlsConfig {
            enabled: true,
            cert_file: paths.server_cert.to_str().unwrap().to_string(),
            key_file: paths.server_key.to_str().unwrap().to_string(),
            ca_file: String::new(),
            require_client_cert: false,
        },
    };
    let mut slave = SlaveServer::new(transport);
    slave.add_station(Station::with_default_points(1, "TLS Test", 2)).await.unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    // Start packet capture
    let mut cap = capture::start("tls_handshake_one_way", port)
        .expect("failed to start capture");
    sleep(Duration::from_millis(500)).await;

    // Connect master with TLS, trusting our CA
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        common_address: 1,
        tls: TlsConfig {
            enabled: true,
            ca_file: paths.ca_cert.to_str().unwrap().to_string(),
            cert_file: String::new(),
            key_file: String::new(),
            accept_invalid_certs: false,
        },
        ..Default::default()
    };
    let mut master = MasterConnection::new(config);
    let connect_result = master.connect().await;
    assert!(connect_result.is_ok(), "TLS connection should succeed: {:?}", connect_result.err());
    sleep(Duration::from_millis(500)).await;

    // Disconnect and stop capture
    master.disconnect().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    slave.stop().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    cap.stop().expect("failed to stop capture");

    // Protocol assertions
    capture::assert_tls_encrypted(&cap.pcap_path, port);
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --package iec104sim-core --test tls_e2e test_tls_handshake_one_way -- --nocapture`
Expected: PASS — TLS handshake succeeds, pcap contains ClientHello/ServerHello, no plaintext IEC 104 leaks.

If it fails with a connection error, check:
- Certificate SAN includes `127.0.0.1`
- rcgen API compatibility (check `rcgen` 0.13 docs if types changed)
- BPF permissions for tcpdump

- [ ] **Step 3: Commit**

```bash
git add crates/iec104sim-core/tests/tls_e2e.rs
git commit -m "test: add one-way TLS handshake e2e test"
```

---

### Task 5: Write test_tls_handshake_mtls

**Files:**
- Modify: `crates/iec104sim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Write the mTLS handshake test**

Append to `tls_e2e.rs`:

```rust
// =========================================================================
// Test: Mutual TLS handshake (server + client auth)
// =========================================================================
#[tokio::test]
async fn test_tls_handshake_mtls() {
    if !check_tools_available() { return; }

    let port = free_port();
    let certs = cert_gen::generate();
    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());

    // Start slave with TLS + client cert settings
    // Note: native_tls TlsAcceptor does not enforce require_client_cert
    // on all platforms. This test verifies the master CAN present a client
    // cert and the connection works with mTLS configuration on both sides.
    let transport = SlaveTransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        tls: SlaveTlsConfig {
            enabled: true,
            cert_file: paths.server_cert.to_str().unwrap().to_string(),
            key_file: paths.server_key.to_str().unwrap().to_string(),
            ca_file: paths.ca_cert.to_str().unwrap().to_string(),
            require_client_cert: true,
        },
    };
    let mut slave = SlaveServer::new(transport);
    slave.add_station(Station::with_default_points(1, "mTLS Test", 2)).await.unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    // Start packet capture
    let mut cap = capture::start("tls_handshake_mtls", port)
        .expect("failed to start capture");
    sleep(Duration::from_millis(500)).await;

    // Connect master with TLS + client certificate
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        common_address: 1,
        tls: TlsConfig {
            enabled: true,
            ca_file: paths.ca_cert.to_str().unwrap().to_string(),
            cert_file: paths.client_cert.to_str().unwrap().to_string(),
            key_file: paths.client_key.to_str().unwrap().to_string(),
            accept_invalid_certs: false,
        },
        ..Default::default()
    };
    let mut master = MasterConnection::new(config);
    let connect_result = master.connect().await;
    assert!(connect_result.is_ok(), "mTLS connection should succeed: {:?}", connect_result.err());
    sleep(Duration::from_millis(500)).await;

    // Disconnect and stop capture
    master.disconnect().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    slave.stop().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    cap.stop().expect("failed to stop capture");

    // Protocol assertions
    capture::assert_tls_encrypted(&cap.pcap_path, port);
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --package iec104sim-core --test tls_e2e test_tls_handshake_mtls -- --nocapture`
Expected: PASS — mTLS connection succeeds, pcap assertions pass.

- [ ] **Step 3: Commit**

```bash
git add crates/iec104sim-core/tests/tls_e2e.rs
git commit -m "test: add mutual TLS handshake e2e test"
```

---

### Task 6: Write test_tls_full_protocol

**Files:**
- Modify: `crates/iec104sim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Write the full protocol test over one-way TLS**

This test verifies GI, spontaneous change-of-state, and control commands all work over TLS. Append to `tls_e2e.rs`:

```rust
// =========================================================================
// Test: Full IEC 104 protocol over one-way TLS
//   1. General Interrogation
//   2. Spontaneous (change-of-state)
//   3. Control command (single point)
// =========================================================================
#[tokio::test]
async fn test_tls_full_protocol() {
    if !check_tools_available() { return; }

    let port = free_port();
    let certs = cert_gen::generate();
    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());

    // Start slave with TLS, add specific data points
    let transport = SlaveTransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        tls: SlaveTlsConfig {
            enabled: true,
            cert_file: paths.server_cert.to_str().unwrap().to_string(),
            key_file: paths.server_key.to_str().unwrap().to_string(),
            ca_file: String::new(),
            require_client_cert: false,
        },
    };
    let mut slave = SlaveServer::new(transport);
    let mut station = Station::new(1, "TLS Protocol Test");
    station.batch_add_points(100, 1, AsduTypeId::MSpNa1, "SP").unwrap();
    station.batch_add_points(200, 1, AsduTypeId::MMeNc1, "FL").unwrap();
    slave.add_station(station).await.unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    // Start packet capture
    let mut cap = capture::start("tls_full_protocol", port)
        .expect("failed to start capture");
    sleep(Duration::from_millis(500)).await;

    // Connect master with TLS
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        common_address: 1,
        tls: TlsConfig {
            enabled: true,
            ca_file: paths.ca_cert.to_str().unwrap().to_string(),
            cert_file: String::new(),
            key_file: String::new(),
            accept_invalid_certs: false,
        },
        ..Default::default()
    };
    let mut master = MasterConnection::new(config);
    master.connect().await.unwrap();
    sleep(Duration::from_millis(500)).await;

    // --- 1. General Interrogation ---
    master.send_interrogation(1).await.unwrap();
    sleep(Duration::from_millis(2000)).await;

    {
        let data = master.received_data.read().await;
        assert!(
            data.get(100, AsduTypeId::MSpNa1).is_some(),
            "IOA=100 (SP) should exist after GI"
        );
        assert!(
            data.get(200, AsduTypeId::MMeNc1).is_some(),
            "IOA=200 (Float) should exist after GI"
        );
    }

    // --- 2. Spontaneous (Change-of-State) ---
    // Modify data point on slave side, then trigger spontaneous send
    {
        let mut stations = slave.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        let point = st.data_points.get_mut(100, AsduTypeId::MSpNa1).unwrap();
        point.value = DataPointValue::SinglePoint { value: true };
    }
    slave.queue_spontaneous(1, &[(100, AsduTypeId::MSpNa1)]).await;
    sleep(Duration::from_millis(2000)).await;

    {
        let data = master.received_data.read().await;
        let point = data.get(100, AsduTypeId::MSpNa1).unwrap();
        assert_eq!(
            point.value,
            DataPointValue::SinglePoint { value: true },
            "Master should receive spontaneous update: SP=true"
        );
    }

    // --- 3. Control Command (single point) ---
    // Send single command to IOA=100, value=false (to toggle it back)
    master.send_single_command(100, false, false, 1).await.unwrap();
    sleep(Duration::from_millis(2000)).await;

    {
        let stations = slave.stations.read().await;
        let point = stations.get(&1).unwrap().data_points.get(100, AsduTypeId::MSpNa1).unwrap();
        assert_eq!(
            point.value,
            DataPointValue::SinglePoint { value: false },
            "Slave data point should be updated by control command"
        );
    }

    {
        let data = master.received_data.read().await;
        let point = data.get(100, AsduTypeId::MSpNa1).unwrap();
        assert_eq!(
            point.value,
            DataPointValue::SinglePoint { value: false },
            "Master should see control writeback via COT=3"
        );
    }

    // Teardown
    master.disconnect().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    slave.stop().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    cap.stop().expect("failed to stop capture");

    // Protocol assertions — entire session should be encrypted
    capture::assert_tls_encrypted(&cap.pcap_path, port);
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --package iec104sim-core --test tls_e2e test_tls_full_protocol -- --nocapture`
Expected: PASS — GI populates data, spontaneous update received, control command works, all over encrypted TLS.

- [ ] **Step 3: Commit**

```bash
git add crates/iec104sim-core/tests/tls_e2e.rs
git commit -m "test: add full IEC 104 protocol over TLS e2e test"
```

---

### Task 7: Write test_tls_mtls_full_protocol

**Files:**
- Modify: `crates/iec104sim-core/tests/tls_e2e.rs`

- [ ] **Step 1: Write the full protocol test over mTLS**

Append to `tls_e2e.rs`:

```rust
// =========================================================================
// Test: Full IEC 104 protocol over mutual TLS
//   Same functional flow as test_tls_full_protocol but with mTLS.
// =========================================================================
#[tokio::test]
async fn test_tls_mtls_full_protocol() {
    if !check_tools_available() { return; }

    let port = free_port();
    let certs = cert_gen::generate();
    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());

    // Start slave with mTLS configuration
    let transport = SlaveTransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        tls: SlaveTlsConfig {
            enabled: true,
            cert_file: paths.server_cert.to_str().unwrap().to_string(),
            key_file: paths.server_key.to_str().unwrap().to_string(),
            ca_file: paths.ca_cert.to_str().unwrap().to_string(),
            require_client_cert: true,
        },
    };
    let mut slave = SlaveServer::new(transport);
    let mut station = Station::new(1, "mTLS Protocol Test");
    station.batch_add_points(100, 1, AsduTypeId::MSpNa1, "SP").unwrap();
    station.batch_add_points(200, 1, AsduTypeId::MMeNc1, "FL").unwrap();
    slave.add_station(station).await.unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    // Start packet capture
    let mut cap = capture::start("tls_mtls_full_protocol", port)
        .expect("failed to start capture");
    sleep(Duration::from_millis(500)).await;

    // Connect master with mTLS (client cert provided)
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        common_address: 1,
        tls: TlsConfig {
            enabled: true,
            ca_file: paths.ca_cert.to_str().unwrap().to_string(),
            cert_file: paths.client_cert.to_str().unwrap().to_string(),
            key_file: paths.client_key.to_str().unwrap().to_string(),
            accept_invalid_certs: false,
        },
        ..Default::default()
    };
    let mut master = MasterConnection::new(config);
    master.connect().await.unwrap();
    sleep(Duration::from_millis(500)).await;

    // --- 1. General Interrogation ---
    master.send_interrogation(1).await.unwrap();
    sleep(Duration::from_millis(2000)).await;

    {
        let data = master.received_data.read().await;
        assert!(
            data.get(100, AsduTypeId::MSpNa1).is_some(),
            "IOA=100 (SP) should exist after GI over mTLS"
        );
        assert!(
            data.get(200, AsduTypeId::MMeNc1).is_some(),
            "IOA=200 (Float) should exist after GI over mTLS"
        );
    }

    // --- 2. Spontaneous (Change-of-State) ---
    {
        let mut stations = slave.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        let point = st.data_points.get_mut(100, AsduTypeId::MSpNa1).unwrap();
        point.value = DataPointValue::SinglePoint { value: true };
    }
    slave.queue_spontaneous(1, &[(100, AsduTypeId::MSpNa1)]).await;
    sleep(Duration::from_millis(2000)).await;

    {
        let data = master.received_data.read().await;
        let point = data.get(100, AsduTypeId::MSpNa1).unwrap();
        assert_eq!(
            point.value,
            DataPointValue::SinglePoint { value: true },
            "Master should receive spontaneous update over mTLS"
        );
    }

    // --- 3. Control Command ---
    master.send_single_command(100, false, false, 1).await.unwrap();
    sleep(Duration::from_millis(2000)).await;

    {
        let stations = slave.stations.read().await;
        let point = stations.get(&1).unwrap().data_points.get(100, AsduTypeId::MSpNa1).unwrap();
        assert_eq!(
            point.value,
            DataPointValue::SinglePoint { value: false },
            "Slave data point should be updated by control over mTLS"
        );
    }

    {
        let data = master.received_data.read().await;
        let point = data.get(100, AsduTypeId::MSpNa1).unwrap();
        assert_eq!(
            point.value,
            DataPointValue::SinglePoint { value: false },
            "Master should see control writeback over mTLS"
        );
    }

    // Teardown
    master.disconnect().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    slave.stop().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    cap.stop().expect("failed to stop capture");

    // Protocol assertions
    capture::assert_tls_encrypted(&cap.pcap_path, port);
}
```

- [ ] **Step 2: Run all TLS tests**

Run: `cargo test --package iec104sim-core --test tls_e2e -- --nocapture`
Expected: All 5 tests pass:
- `test_cert_generation` — PASS
- `test_tls_handshake_one_way` — PASS
- `test_tls_handshake_mtls` — PASS
- `test_tls_full_protocol` — PASS
- `test_tls_mtls_full_protocol` — PASS

- [ ] **Step 3: Verify pcap files were created**

Run: `ls -la crates/iec104sim-core/tests/pcap/`
Expected: 4 pcap files (one per capture-enabled test)

- [ ] **Step 4: Commit**

```bash
git add crates/iec104sim-core/tests/tls_e2e.rs
git commit -m "test: add mTLS full protocol e2e test, complete TLS test suite"
```

---

### Task 8: Final Verification and Cleanup

**Files:**
- No new files

- [ ] **Step 1: Run the full test suite (existing + new)**

Run: `cargo test --package iec104sim-core -- --nocapture 2>&1 | tail -20`
Expected: All existing tests in `control_e2e.rs` still pass, all new TLS tests pass. No regressions.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --package iec104sim-core --tests -- -D warnings 2>&1 | tail -10`
Expected: No warnings

- [ ] **Step 3: Fix any clippy warnings**

If clippy reports warnings, fix them in `tls_e2e.rs` and re-run.

- [ ] **Step 4: Verify .gitignore works**

Run: `git status`
Expected: `tests/pcap/` directory and its contents should NOT appear in untracked files.

- [ ] **Step 5: Final commit (if any fixes)**

```bash
git add -A && git commit -m "test: cleanup and final TLS test suite verification"
```
