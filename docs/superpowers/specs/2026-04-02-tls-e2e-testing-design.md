# TLS End-to-End Testing Design

## Overview

为 IEC 60870-5-104 Simulator 新增 TLS 端到端测试，验证主站和子站在 TLS（单向 + mTLS）下的连接和协议功能正确性，同时通过抓包自动断言 TLS 加密生效。

## Decisions

| 项目 | 选择 | 理由 |
|------|------|------|
| 抓包方式 | 外部 `tcpdump`/`tshark` 子进程 | 生成标准 pcap，灵活且无 Rust pcap 依赖 |
| 证书生成 | `rcgen` 动态生成 | 零维护，无过期问题，测试自包含 |
| TLS 模式 | 单向 TLS + 双向 mTLS | 覆盖两种实际部署场景 |
| 验证方式 | 自动断言 + pcap 留档 | CI 可用，同时保留 pcap 供人工调试 |
| 测试组织 | 混合：握手独立 + 功能合并 | 握手验证清晰隔离，功能测试一个 pcap 看完整流程 |
| 验证深度 | 应用层断言 + 协议层验证加密 | 应用层验证功能正确，协议层验证加密生效 |
| TLS 库 | 保留 `native-tls` | 不改变现有实现，协议层不需解密 |
| 测试结构 | 单文件 `tests/tls_e2e.rs` | 与现有 `control_e2e.rs` 风格一致 |

## Architecture

### File Structure

```
crates/iec104sim-core/
├── tests/
│   ├── control_e2e.rs        # existing (unchanged)
│   ├── tls_e2e.rs            # new: all TLS e2e tests
│   └── pcap/                 # new: packet capture output (gitignored)
├── Cargo.toml                # modified: new dev-dependencies
```

### Module Layout (within tls_e2e.rs)

```
tls_e2e.rs
├── mod cert_gen              # Certificate generation with rcgen
│   ├── struct TestCerts      # PEM strings for CA/server/client
│   ├── fn generate()         # Generate full cert chain
│   └── fn write_to_dir()     # Write PEMs to tempdir, return paths
├── mod capture               # Packet capture control
│   ├── struct PacketCapture  # tcpdump child process + pcap path
│   ├── fn start()            # Launch tcpdump on lo0
│   ├── fn stop()             # SIGTERM + wait
│   └── fn assert_tls_encrypted()  # tshark analysis assertions
├── fn check_tools_available()     # Skip if tcpdump/tshark missing
├── test_tls_handshake_one_way
├── test_tls_handshake_mtls
├── test_tls_full_protocol
└── test_tls_mtls_full_protocol
```

## Certificate Generation Module

### Dependencies

```toml
[dev-dependencies]
rcgen = "0.13"
tempfile = "3"
```

### Generated Certificates

| Certificate | Properties |
|-------------|-----------|
| CA | Self-signed, `is_ca = true`, RSA or ECDSA |
| Server | Signed by CA, SAN: `localhost` + `127.0.0.1` |
| Client | Signed by CA (used in mTLS tests only) |

### Data Flow

```
rcgen::generate() -> TestCerts (PEM strings)
    -> write_to_dir(TempDir) -> file paths
        -> SlaveTlsConfig { cert_file, key_file, ca_file }
        -> TlsConfig { ca_file, cert_file, key_file }
```

`TempDir` lifetime is bound to the test function; cleanup is automatic.

## Packet Capture Module

### Start Capture

```
tcpdump -i lo0 -w <pcap_path> port <port> &
sleep(500ms)  // wait for tcpdump readiness
```

- pcap files saved to `tests/pcap/<test_name>_<timestamp>.pcap`
- macOS loopback interface: `lo0`

### Stop Capture

```
SIGTERM -> tcpdump child
waitpid -> ensure pcap is flushed
```

### TLS Assertions (via tshark)

Three checks per pcap:

1. **TLS handshake present:**
   ```
   tshark -r <pcap> -Y "tls.handshake" -T fields -e tls.handshake.type
   ```
   Assert output contains `1` (ClientHello) and `2` (ServerHello).

2. **No plaintext IEC 104 leakage:**
   ```
   tshark -r <pcap> -Y "iec60870_104" -T fields -e frame.number
   ```
   Assert output is empty.

3. **Encrypted application data present:**
   ```
   tshark -r <pcap> -Y "tls.record.content_type == 23" -T fields -e frame.number
   ```
   Assert output is non-empty (content_type 23 = Application Data).

### pcap Retention

- Always retained after test (pass or fail)
- On assertion failure: pcap path printed to stderr for debugging
- Directory `tests/pcap/` is gitignored

## Test Cases

### 1. test_tls_handshake_one_way

**Purpose:** Verify one-way TLS (server auth only) handshake succeeds.

**Setup:**
- Generate CA + Server certs
- Start slave: `tls.enabled = true`, `require_client_cert = false`
- Start tcpdump

**Actions:**
- Master connects with `tls.enabled = true`, `ca_file = CA cert`, no client cert

**Assertions:**
- Application: Connection established successfully
- Protocol: TLS handshake present, no plaintext leakage

**Teardown:** Disconnect, stop tcpdump

### 2. test_tls_handshake_mtls

**Purpose:** Verify mutual TLS (client + server auth) handshake succeeds.

**Setup:**
- Generate CA + Server + Client certs
- Start slave: `tls.enabled = true`, `require_client_cert = true`, `ca_file = CA cert`
- Start tcpdump

**Actions:**
- Master connects with `tls.enabled = true`, CA + client cert + client key

**Assertions:**
- Application: Connection established successfully
- Protocol: TLS handshake present (including Certificate messages), no plaintext leakage

**Teardown:** Disconnect, stop tcpdump

### 3. test_tls_full_protocol

**Purpose:** Verify GI, spontaneous, and control commands work correctly over one-way TLS.

**Setup:**
- Generate CA + Server certs
- Start slave with data points: M_SP_NA (IOA=100), M_ME_NC (IOA=200)
- Start tcpdump
- Master connects (one-way TLS)

**Actions (sequential):**

1. **General Interrogation:**
   - Master sends GI command
   - Assert: Master receives data point sync (IOA=100, IOA=200 values)

2. **Spontaneous (Change-of-State):**
   - Modify slave data point value (IOA=100)
   - Assert: Master receives COT=3 spontaneous frame with updated value

3. **Control Command:**
   - Master sends single command (C_SC_NA_1) to IOA=100
   - Assert: Slave data point updated, master receives activation confirm (COT=7) and termination (COT=10)

**Protocol Assertions:**
- Full session encrypted: TLS handshake present, encrypted app data present, no plaintext IEC 104

**Teardown:** Disconnect, stop tcpdump

### 4. test_tls_mtls_full_protocol

**Purpose:** Same as #3 but over mutual TLS.

Identical functional flow and assertions as test #3, with mTLS connection setup (server + client certs, `require_client_cert = true`).

Verifies that mTLS does not interfere with protocol functionality.

## Port Allocation

Each test uses a random available port (consistent with existing `control_e2e.rs` pattern) to allow parallel execution.

## Tool Prerequisite Check

Each test begins with:
```rust
fn check_tools_available() -> bool {
    Command::new("tcpdump").arg("--version").output().is_ok()
        && Command::new("tshark").arg("--version").output().is_ok()
}
```

If tools are missing, the test prints a warning and returns `Ok(())` (skip), avoiding hard failure on environments without capture tools.

## Dependencies Summary

### New dev-dependencies (Cargo.toml)

```toml
[dev-dependencies]
rcgen = "0.13"
tempfile = "3"
```

### External tools (runtime)

- `tcpdump` — packet capture (pre-installed on macOS)
- `tshark` — pcap analysis (install via `brew install wireshark`)

### .gitignore addition

```
tests/pcap/
```

## Error Handling

- tcpdump/tshark launch failure: test skipped with warning
- Certificate generation failure: test fails with descriptive error
- TLS handshake failure: test fails (this IS what we're testing)
- pcap analysis failure: test fails, prints pcap path for manual inspection

## Scope Exclusions

- No TLS library migration (keep native-tls)
- No TLS traffic decryption (no SSLKEYLOGFILE)
- No performance/load testing
- No certificate expiry/revocation testing
- No modification to existing `control_e2e.rs`
- No changes to production code (slave.rs, master.rs)
