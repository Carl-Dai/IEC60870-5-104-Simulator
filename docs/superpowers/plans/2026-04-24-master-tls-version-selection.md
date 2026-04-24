# Master TLS Version Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let master-side users pick TLS version (`Auto` / `Tls12Only` / `Tls13Only`) from the "New Connection" dialog; the setting survives persistence round-trip and drives the `native_tls::TlsConnector` builder.

**Architecture:** Add a `TlsVersionPolicy` enum alongside `TlsConfig`; serde-default to `Auto` for backward compatibility. `create_tls_stream` branches on the policy (for `Tls13Only` explicitly set both `min` and `max` to `Tlsv13` to bypass the macOS Security Framework quirk that silently downgrades `max=Tlsv13` when `min≠Tlsv13`). Tauri command accepts an optional string, maps it to the enum (unknown/missing → `Auto`). Frontend adds a `<select>` bound to `newConnForm.tls_version`.

**Tech Stack:** Rust 1.x + `native-tls` 0.2 + `serde` + Tauri v2 + Vue 3 `<script setup>` + `vue-tsc` + `vite`.

**Spec:** `docs/superpowers/specs/2026-04-24-master-tls-version-selection-design.md`

---

## File Structure

**Create:**
- `crates/iec104sim-core/tests/tls_version_negotiation.rs` — new e2e test covering 3 positive + 1 negative TLS version paths

**Modify:**
- `crates/iec104sim-core/src/master.rs` — add enum; extend `TlsConfig`; branch in `create_tls_stream`
- `crates/iec104master-app/src/commands.rs` — extend `CreateConnectionRequest` + parse in `create_connection`
- `master-frontend/src/components/Toolbar.vue` — extend `newConnForm`, payload to `invoke`, add `<select>` input

**Do not touch:**
- `crates/iec104sim-core/src/slave.rs` and `crates/iec104sim-core/tests/tls_e2e.rs` — spec explicitly scopes change to master only.

Each file has a single reason to change; tasks stay within those boundaries.

---

### Task 1: Add `TlsVersionPolicy` enum and extend `TlsConfig`

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs:39-61`
- Test: `crates/iec104sim-core/src/master.rs` (inline `#[cfg(test)] mod tests`, existing at bottom of file)

- [ ] **Step 1: Write the failing unit tests**

Open `crates/iec104sim-core/src/master.rs` and locate the existing `mod tests` block near the bottom (look for `fn test_tls_config_default`). Add two new tests right after it, inside the same `mod tests`:

```rust
    #[test]
    fn test_tls_version_policy_default_is_auto() {
        let v = TlsVersionPolicy::default();
        assert_eq!(v, TlsVersionPolicy::Auto);
    }

    #[test]
    fn test_tls_config_default_version_is_auto() {
        let cfg = TlsConfig::default();
        assert_eq!(cfg.version, TlsVersionPolicy::Auto);
    }

    #[test]
    fn test_tls_version_policy_serde_snake_case() {
        // snake_case renames must match the strings the Tauri layer parses.
        let auto = serde_json::to_string(&TlsVersionPolicy::Auto).unwrap();
        let v12  = serde_json::to_string(&TlsVersionPolicy::Tls12Only).unwrap();
        let v13  = serde_json::to_string(&TlsVersionPolicy::Tls13Only).unwrap();
        assert_eq!(auto, "\"auto\"");
        assert_eq!(v12, "\"tls12_only\"");
        assert_eq!(v13, "\"tls13_only\"");
    }

    #[test]
    fn test_tls_config_deserialize_without_version_field_defaults_to_auto() {
        // Backward compat: old persisted JSON has no `version` field.
        let json = r#"{"enabled": true}"#;
        let cfg: TlsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.version, TlsVersionPolicy::Auto);
        assert!(cfg.enabled);
    }
```

Also check that `serde_json` is in `dev-dependencies` of `crates/iec104sim-core/Cargo.toml`. Run `grep -n serde_json crates/iec104sim-core/Cargo.toml`. If missing, add it under `[dev-dependencies]`:

```toml
serde_json = "1"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p iec104sim-core --lib tls_version`
Expected: `error[E0433]: failed to resolve: use of undeclared type TlsVersionPolicy` (or similar — the type does not exist yet).

- [ ] **Step 3: Add the enum and extend `TlsConfig`**

Edit `crates/iec104sim-core/src/master.rs` around line 39 (the existing `TlsConfig` struct). Insert `TlsVersionPolicy` immediately before `TlsConfig`, and add a `version` field at the bottom of `TlsConfig`:

```rust
/// Strategy for choosing the TLS protocol version on the client side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsVersionPolicy {
    /// Negotiate automatically (min = TLS 1.2, no max cap).
    #[default]
    Auto,
    /// Pin to TLS 1.2 (min = max = TLS 1.2).
    Tls12Only,
    /// Pin to TLS 1.3 (min = max = TLS 1.3).
    Tls13Only,
}

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
    /// Path to client PKCS#12 bundle for mutual TLS (preferred on macOS)
    #[serde(default)]
    pub pkcs12_file: String,
    /// Password for the PKCS#12 bundle
    #[serde(default)]
    pub pkcs12_password: String,
    /// Accept invalid/self-signed certificates (for testing)
    #[serde(default)]
    pub accept_invalid_certs: bool,
    /// TLS version policy. Defaults to `Auto` (min=1.2, no max cap).
    #[serde(default)]
    pub version: TlsVersionPolicy,
}
```

(Keep the existing comment/doc style. The field order change only appends `version` at the end, so struct-literal call sites that name fields remain valid.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p iec104sim-core --lib tls_version`
Expected: 4 tests pass (`test_tls_version_policy_default_is_auto`, `test_tls_config_default_version_is_auto`, `test_tls_version_policy_serde_snake_case`, `test_tls_config_deserialize_without_version_field_defaults_to_auto`).

Also run full core test suite to ensure the new field didn't break any existing struct literal:

```
cargo test -p iec104sim-core --lib
```

If any caller constructs `TlsConfig { ... }` without `..Default::default()` and without `version`, fix it by adding `version: TlsVersionPolicy::default(),`. Likely none in `core` itself, but check `iec104master-app` once Task 4 is reached.

- [ ] **Step 5: Commit**

```bash
git add crates/iec104sim-core/src/master.rs crates/iec104sim-core/Cargo.toml
git commit -m "feat(core): add TlsVersionPolicy enum and TlsConfig.version field"
```

---

### Task 2: Branch `create_tls_stream` on policy

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs:414-462` (`create_tls_stream` method)

Behaviour is end-to-end only; this task is NOT test-first — Task 3 (integration tests) drives the verification. Step 1 below writes the Task 3 test first in principle, but for task-decomposition clarity we implement the branch now and run Task 3 after Task 4 (Tauri) is untouched by this. The integration tests in Task 3 are the gate.

- [ ] **Step 1: Replace the hardcoded min-version call with a policy-driven match**

In `crates/iec104sim-core/src/master.rs`, find the block that reads:

```rust
        // Set minimum TLS version to 1.2 (IEC 62351 requirement)
        builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
```

Replace with:

```rust
        // Apply configured TLS version policy. For `Tls13Only` we pin both ends
        // explicitly — macOS Security Framework silently downgrades `max=Tlsv13`
        // to 1.2 if `min != Tlsv13` (see native-tls 0.2.18 imp/security_framework.rs).
        match self.config.tls.version {
            TlsVersionPolicy::Auto => {
                builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
            }
            TlsVersionPolicy::Tls12Only => {
                builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
                builder.max_protocol_version(Some(native_tls::Protocol::Tlsv12));
            }
            TlsVersionPolicy::Tls13Only => {
                builder.min_protocol_version(Some(native_tls::Protocol::Tlsv13));
                builder.max_protocol_version(Some(native_tls::Protocol::Tlsv13));
            }
        }
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p iec104sim-core`
Expected: clean compile (no errors, no warnings related to TLS).

- [ ] **Step 3: Commit**

```bash
git add crates/iec104sim-core/src/master.rs
git commit -m "feat(core): honour TlsVersionPolicy in create_tls_stream"
```

---

### Task 3: Add e2e TLS version negotiation tests

**Files:**
- Create: `crates/iec104sim-core/tests/tls_version_negotiation.rs`

This test file reuses the `cert_gen` helper pattern from `tls_e2e.rs`. For the negative case we spin up a bare `native_tls::TlsAcceptor` directly (not `SlaveServer`) so that we can force TLS 1.3-only without modifying slave code — the spec forbids slave changes.

- [ ] **Step 1: Write the full test file**

Create `crates/iec104sim-core/tests/tls_version_negotiation.rs`:

```rust
//! End-to-end tests for `TlsVersionPolicy` in the master connector.
//!
//! Covers:
//!   1. Auto          vs default slave (handshake succeeds)
//!   2. Tls12Only     vs default slave (handshake succeeds)
//!   3. Tls13Only     vs default slave (handshake succeeds)
//!   4. Tls12Only     vs TLS-1.3-only server (handshake fails)
//!
//! The negative case uses a raw `native_tls::TlsAcceptor` (not `SlaveServer`)
//! so we can pin the server to TLS 1.3 without modifying slave code.

use iec104sim_core::master::{MasterConfig, MasterConnection, TlsConfig, TlsVersionPolicy};
use iec104sim_core::slave::{SlaveServer, SlaveTlsConfig, SlaveTransportConfig, Station};
use std::io::Read;
use std::net::TcpListener;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// --- Inline cert helpers: copy of the cert_gen module used by tls_e2e.rs ---
// Keeping a local copy avoids exposing tls_e2e internals as pub. Small enough
// to duplicate; refactoring to a shared `mod common` is out of scope here.
mod cert_gen {
    use std::path::{Path, PathBuf};

    pub const PKCS12_PASS: &str = "iec104test";

    pub struct TestCerts {
        pub ca_cert_pem: String,
        pub server_cert_pem: String,
        pub server_key_pem: String,
        pub client_cert_pem: String,
        pub client_key_pem: String,
    }

    pub struct CertPaths {
        pub ca_cert: PathBuf,
        pub server_pkcs12: PathBuf,
        pub client_pkcs12: PathBuf,
    }

    pub fn generate() -> TestCerts {
        use rcgen::{
            BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
            KeyUsagePurpose, SanType,
        };
        let mut ca_params = CertificateParams::new(vec!["IEC104 Test CA".into()]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        ca_params.distinguished_name.push(DnType::CommonName, "IEC104 Test CA");
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();

        let mut srv = CertificateParams::new(vec!["localhost".into()]).unwrap();
        srv.subject_alt_names = vec![
            SanType::DnsName("localhost".try_into().unwrap()),
            SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
        ];
        srv.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        srv.distinguished_name.push(DnType::CommonName, "IEC104 Test Server");
        let srv_key = KeyPair::generate().unwrap();
        let srv_cert = srv.signed_by(&srv_key, &ca_cert, &ca_key).unwrap();

        let mut cli = CertificateParams::new(vec!["IEC104 Test Client".into()]).unwrap();
        cli.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        cli.distinguished_name.push(DnType::CommonName, "IEC104 Test Client");
        let cli_key = KeyPair::generate().unwrap();
        let cli_cert = cli.signed_by(&cli_key, &ca_cert, &ca_key).unwrap();

        TestCerts {
            ca_cert_pem: ca_cert.pem(),
            server_cert_pem: srv_cert.pem(),
            server_key_pem: srv_key.serialize_pem(),
            client_cert_pem: cli_cert.pem(),
            client_key_pem: cli_key.serialize_pem(),
        }
    }

    pub fn write_to_dir(certs: &TestCerts, dir: &Path) -> CertPaths {
        let ca_cert = dir.join("ca.pem");
        let server_cert = dir.join("server.pem");
        let server_key = dir.join("server-key.pem");
        let server_pkcs12 = dir.join("server.p12");
        let client_cert = dir.join("client.pem");
        let client_key = dir.join("client-key.pem");
        let client_pkcs12 = dir.join("client.p12");
        std::fs::write(&ca_cert, &certs.ca_cert_pem).unwrap();
        std::fs::write(&server_cert, &certs.server_cert_pem).unwrap();
        std::fs::write(&server_key, &certs.server_key_pem).unwrap();
        std::fs::write(&client_cert, &certs.client_cert_pem).unwrap();
        std::fs::write(&client_key, &certs.client_key_pem).unwrap();
        make_pkcs12(&server_cert, &server_key, &server_pkcs12, PKCS12_PASS);
        make_pkcs12(&client_cert, &client_key, &client_pkcs12, PKCS12_PASS);
        CertPaths { ca_cert, server_pkcs12, client_pkcs12 }
    }

    fn make_pkcs12(cert: &Path, key: &Path, out: &Path, password: &str) {
        let st = std::process::Command::new("openssl")
            .args([
                "pkcs12", "-export",
                "-in", cert.to_str().unwrap(),
                "-inkey", key.to_str().unwrap(),
                "-out", out.to_str().unwrap(),
                "-passout", &format!("pass:{}", password),
            ])
            .status()
            .expect("openssl not found — required for PKCS#12 generation in tests");
        assert!(st.success(), "openssl pkcs12 export failed");
    }
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Spin up a `SlaveServer` with default TLS config (min=1.2, no max cap).
/// Returns (server, port, tmpdir, client_pkcs12_path, ca_path).
async fn spawn_default_tls_slave() -> (SlaveServer, u16, tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let certs = cert_gen::generate();
    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());
    let port = free_port();

    let transport = SlaveTransportConfig {
        bind_address: "127.0.0.1".into(),
        port,
        tls: SlaveTlsConfig {
            enabled: true,
            pkcs12_file: paths.server_pkcs12.to_string_lossy().into(),
            pkcs12_password: cert_gen::PKCS12_PASS.into(),
            ..Default::default()
        },
    };
    let mut slave = SlaveServer::new(transport);
    slave.add_station(Station::with_default_points(1, "v", 1)).await.unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    let client_p12 = paths.client_pkcs12.clone();
    let ca = paths.ca_cert.clone();
    (slave, port, tmp, client_p12, ca)
}

fn master_config(port: u16, version: TlsVersionPolicy, client_p12: &std::path::Path, ca: &std::path::Path) -> MasterConfig {
    MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        timeout_ms: 3000,
        tls: TlsConfig {
            enabled: true,
            ca_file: ca.to_string_lossy().into(),
            pkcs12_file: client_p12.to_string_lossy().into(),
            pkcs12_password: cert_gen::PKCS12_PASS.into(),
            accept_invalid_certs: true, // self-signed test CA
            version,
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn master_auto_handshakes_with_default_slave() {
    let (mut slave, port, _tmp, client_p12, ca) = spawn_default_tls_slave().await;
    let mut master = MasterConnection::new(master_config(port, TlsVersionPolicy::Auto, &client_p12, &ca));
    master.connect().await.expect("Auto handshake should succeed");
    sleep(Duration::from_millis(200)).await;
    master.disconnect().await.ok();
    slave.stop().await.ok();
}

#[tokio::test]
async fn master_tls12_only_handshakes_with_default_slave() {
    let (mut slave, port, _tmp, client_p12, ca) = spawn_default_tls_slave().await;
    let mut master = MasterConnection::new(master_config(port, TlsVersionPolicy::Tls12Only, &client_p12, &ca));
    master.connect().await.expect("Tls12Only handshake should succeed");
    sleep(Duration::from_millis(200)).await;
    master.disconnect().await.ok();
    slave.stop().await.ok();
}

#[tokio::test]
async fn master_tls13_only_handshakes_with_default_slave() {
    let (mut slave, port, _tmp, client_p12, ca) = spawn_default_tls_slave().await;
    let mut master = MasterConnection::new(master_config(port, TlsVersionPolicy::Tls13Only, &client_p12, &ca));
    master.connect().await.expect("Tls13Only handshake should succeed");
    sleep(Duration::from_millis(200)).await;
    master.disconnect().await.ok();
    slave.stop().await.ok();
}

#[tokio::test]
async fn master_tls12_only_fails_against_tls13_only_server() {
    // Build a bare TLS 1.3-only acceptor using native_tls directly.
    let certs = cert_gen::generate();
    let tmp = tempfile::tempdir().unwrap();
    let paths = cert_gen::write_to_dir(&certs, tmp.path());
    let port = free_port();
    let server_pkcs12 = std::fs::read(&paths.server_pkcs12).unwrap();
    let identity = native_tls::Identity::from_pkcs12(&server_pkcs12, cert_gen::PKCS12_PASS).unwrap();
    let acceptor = native_tls::TlsAcceptor::builder(identity)
        .min_protocol_version(Some(native_tls::Protocol::Tlsv13))
        .max_protocol_version(Some(native_tls::Protocol::Tlsv13))
        .build()
        .unwrap();
    let acceptor = Arc::new(acceptor);

    // TCP accept loop: accept once, perform TLS handshake, then drop.
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let acc_clone = acceptor.clone();
    let server_handle = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Handshake is expected to fail; ignore result.
            let _ = acc_clone.accept(stream).map(|mut s| {
                let mut buf = [0u8; 1];
                let _ = s.read(&mut buf);
            });
        }
    });

    // Master pinned to TLS 1.2 should fail handshake.
    let mut master = MasterConnection::new(master_config(port, TlsVersionPolicy::Tls12Only, &paths.client_pkcs12, &paths.ca_cert));
    let result = master.connect().await;
    assert!(result.is_err(), "Tls12Only vs TLS-1.3-only server must fail, got {:?}", result);
    let err = result.err().unwrap();
    let msg = format!("{}", err);
    assert!(
        msg.contains("TLS") || msg.contains("tls") || msg.contains("handshake") || msg.contains("握手"),
        "error should mention TLS/handshake, got: {}", msg
    );

    let _ = server_handle.join();
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test -p iec104sim-core --test tls_version_negotiation -- --test-threads=1`

Expected: 4 passed. (`--test-threads=1` because each test binds a port and spawns a slave server with its own tokio runtime — sequential is simpler and matches `tls_e2e.rs` conventions.)

If `openssl` CLI is missing on the test host, `make_pkcs12` will panic — mirror the existing `tls_e2e.rs` requirement and document in the README if needed. **Do not** skip the precondition check — these tests need real certs.

- [ ] **Step 3: Commit**

```bash
git add crates/iec104sim-core/tests/tls_version_negotiation.rs
git commit -m "test(core): add e2e tests for master TlsVersionPolicy negotiation"
```

---

### Task 4: Tauri command — accept `tls_version` and parse it

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs:24-66` (`CreateConnectionRequest` + `create_connection`)

- [ ] **Step 1: Extend the request struct**

Open `crates/iec104master-app/src/commands.rs`. Find:

```rust
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
```

Add one field at the bottom:

```rust
    /// TLS version policy: "auto" | "tls12_only" | "tls13_only" (default: "auto")
    pub tls_version: Option<String>,
```

- [ ] **Step 2: Add the import**

In the same file, find the line:

```rust
use iec104sim_core::master::{ControlResult, ControlStep, MasterConfig, MasterConnection, TlsConfig};
```

Add `TlsVersionPolicy`:

```rust
use iec104sim_core::master::{ControlResult, ControlStep, MasterConfig, MasterConnection, TlsConfig, TlsVersionPolicy};
```

- [ ] **Step 3: Parse the field when building `TlsConfig`**

Find the `TlsConfig { ... }` literal inside `create_connection`:

```rust
        tls: TlsConfig {
            enabled: request.use_tls.unwrap_or(false),
            ca_file: request.ca_file.unwrap_or_default(),
            cert_file: request.cert_file.unwrap_or_default(),
            key_file: request.key_file.unwrap_or_default(),
            pkcs12_file: String::new(),
            pkcs12_password: String::new(),
            accept_invalid_certs: request.accept_invalid_certs.unwrap_or(false),
        },
```

Replace with:

```rust
        tls: TlsConfig {
            enabled: request.use_tls.unwrap_or(false),
            ca_file: request.ca_file.unwrap_or_default(),
            cert_file: request.cert_file.unwrap_or_default(),
            key_file: request.key_file.unwrap_or_default(),
            pkcs12_file: String::new(),
            pkcs12_password: String::new(),
            accept_invalid_certs: request.accept_invalid_certs.unwrap_or(false),
            version: match request.tls_version.as_deref() {
                Some("tls12_only") => TlsVersionPolicy::Tls12Only,
                Some("tls13_only") => TlsVersionPolicy::Tls13Only,
                _ => TlsVersionPolicy::Auto,
            },
        },
```

- [ ] **Step 4: Compile check**

Run: `cargo check -p iec104master-app`
Expected: clean compile.

- [ ] **Step 5: Run full workspace test (regression guard)**

Run: `cargo test --workspace --lib`
Expected: all green. If any existing `TlsConfig { ... }` literal in the workspace now complains about the missing field, add `version: TlsVersionPolicy::default(),` to those sites. (Primary suspect would be tests under `iec104sim-core/tests/tls_e2e.rs` — they use `TlsConfig { ..., ..Default::default() }` already; should be fine.)

- [ ] **Step 6: Commit**

```bash
git add crates/iec104master-app/src/commands.rs
git commit -m "feat(master-app): accept tls_version in create_connection"
```

---

### Task 5: Frontend — add TLS version selector to New Connection dialog

**Files:**
- Modify: `master-frontend/src/components/Toolbar.vue` (form state around line 18-50, template around line 210-227)

- [ ] **Step 1: Extend `newConnForm` with `tls_version`**

Open `master-frontend/src/components/Toolbar.vue`. Locate the `newConnForm` ref:

```ts
const newConnForm = ref({
  target_address: '127.0.0.1',
  port: 2404,
  common_address: 1,
  use_tls: false,
  ca_file: '',
  cert_file: '',
  key_file: '',
  accept_invalid_certs: false,
})
```

Replace with (adds `tls_version` at the end):

```ts
const newConnForm = ref({
  target_address: '127.0.0.1',
  port: 2404,
  common_address: 1,
  use_tls: false,
  ca_file: '',
  cert_file: '',
  key_file: '',
  accept_invalid_certs: false,
  tls_version: 'auto' as 'auto' | 'tls12_only' | 'tls13_only',
})
```

- [ ] **Step 2: Pass `tls_version` to the backend invoke payload**

Locate the `createConnection` function:

```ts
async function createConnection() {
  try {
    await invoke('create_connection', {
      request: {
        target_address: newConnForm.value.target_address,
        port: newConnForm.value.port,
        common_address: newConnForm.value.common_address,
        use_tls: newConnForm.value.use_tls,
        ca_file: newConnForm.value.ca_file || undefined,
        cert_file: newConnForm.value.cert_file || undefined,
        key_file: newConnForm.value.key_file || undefined,
        accept_invalid_certs: newConnForm.value.accept_invalid_certs,
      }
    })
```

Add `tls_version` to the payload. Only send it when TLS is enabled, otherwise send undefined (backend treats missing as `Auto`):

```ts
async function createConnection() {
  try {
    await invoke('create_connection', {
      request: {
        target_address: newConnForm.value.target_address,
        port: newConnForm.value.port,
        common_address: newConnForm.value.common_address,
        use_tls: newConnForm.value.use_tls,
        ca_file: newConnForm.value.ca_file || undefined,
        cert_file: newConnForm.value.cert_file || undefined,
        key_file: newConnForm.value.key_file || undefined,
        accept_invalid_certs: newConnForm.value.accept_invalid_certs,
        tls_version: newConnForm.value.use_tls ? newConnForm.value.tls_version : undefined,
      }
    })
```

- [ ] **Step 3: Add the `<select>` to the modal template**

In the same file, find the `<template v-if="newConnForm.use_tls">` block (currently contains CA/cert/key file inputs + `accept_invalid_certs` checkbox). Insert a new `<label>` just **before** the CA certificate input:

```vue
          <template v-if="newConnForm.use_tls">
            <label class="form-label">
              TLS 版本
              <select v-model="newConnForm.tls_version" class="form-input">
                <option value="auto">自动</option>
                <option value="tls12_only">仅 TLS 1.2</option>
                <option value="tls13_only">仅 TLS 1.3</option>
              </select>
            </label>
            <label class="form-label">
              CA 证书路径
              ... (existing unchanged) ...
```

The exact existing block to extend looks like:

```vue
          <template v-if="newConnForm.use_tls">
            <label class="form-label">
              CA 证书路径
              <input v-model="newConnForm.ca_file" class="form-input" type="text" placeholder="/path/to/ca.crt" />
            </label>
```

Turn it into:

```vue
          <template v-if="newConnForm.use_tls">
            <label class="form-label">
              TLS 版本
              <select v-model="newConnForm.tls_version" class="form-input">
                <option value="auto">自动</option>
                <option value="tls12_only">仅 TLS 1.2</option>
                <option value="tls13_only">仅 TLS 1.3</option>
              </select>
            </label>
            <label class="form-label">
              CA 证书路径
              <input v-model="newConnForm.ca_file" class="form-input" type="text" placeholder="/path/to/ca.crt" />
            </label>
```

- [ ] **Step 4: Type check the frontend**

Run: `cd master-frontend && npx vue-tsc --noEmit`
Expected: exit code 0, no errors.

- [ ] **Step 5: Build the frontend**

Run: `cd master-frontend && npm run build`
Expected: Vite build succeeds.

- [ ] **Step 6: Commit**

```bash
git add master-frontend/src/components/Toolbar.vue
git commit -m "feat(master-frontend): add TLS version selector to New Connection dialog"
```

---

### Task 6: Full verification

**Files:**
- Run-only: verify nothing broke end-to-end.

- [ ] **Step 1: Workspace compile**

Run: `cargo check --workspace`
Expected: clean.

- [ ] **Step 2: Workspace library tests**

Run: `cargo test --workspace --lib`
Expected: all pass (includes the 4 new unit tests from Task 1).

- [ ] **Step 3: New integration tests**

Run: `cargo test -p iec104sim-core --test tls_version_negotiation -- --test-threads=1`
Expected: 4 passed.

- [ ] **Step 4: Regression: existing TLS e2e**

Run: `cargo test -p iec104sim-core --test tls_e2e -- --test-threads=1`
Expected: 5 passed (same as before; `Auto` default path unchanged).

- [ ] **Step 5: Regression: other integration suites**

Run: `cargo test -p iec104sim-core --test control_e2e && cargo test -p iec104sim-core --test disconnect_detection && cargo test -p iec104sim-core --test overlapping_ioa_interrogation`
Expected: all pass.

- [ ] **Step 6: Master frontend type-check + build**

Run: `cd master-frontend && npx vue-tsc --noEmit && npm run build`
Expected: both succeed.

- [ ] **Step 7: Slave frontend type-check + build (regression guard — must still work since we didn't touch it)**

Run: `cd frontend && npx vue-tsc --noEmit && npm run build`
Expected: both succeed.

- [ ] **Step 8: Smoke-test the UI manually (if possible)**

This step is optional in headless CI, but if a display is available:

```
cd crates/iec104master-app && cargo tauri dev
```

Steps:
1. Click "新建连接" → toggle "启用 TLS" → the "TLS 版本" select should appear with 3 options, default "自动".
2. Change to "仅 TLS 1.3" and try connecting to a TLS 1.2-only server → expect error banner.
3. Reset to "自动" and connect to a normal TLS-enabled slave → expect success.

If headless, skip this step but note it in the commit summary.

- [ ] **Step 9: Final commit (only if any trailing tweaks were needed)**

If steps 1-8 all passed without changes, skip. Otherwise commit the fix as:

```bash
git add <files>
git commit -m "fix: resolve regression found in full verification pass"
```

---

## Out of scope (deferred)

- Slave-side TLS version policy (user requested master only; spec forbids).
- Showing the negotiated TLS version in the About dialog (`native-tls` does not expose it).
- Mixed ranges like `Tls12Plus` / `Tls13Plus` (YAGNI; covered by enum extension later).
- Bumping workspace version to v1.0.6 — handled by the `/release` skill in a separate commit after this plan lands.

---

## Self-Review Notes

**Spec coverage:**
- §3 data model → Task 1
- §4 connector behaviour → Task 2
- §5 UI → Task 5
- §6 Tauri bridging → Task 4
- §7 error handling → covered by existing `TlsError` path; Task 3 negative test verifies
- §8 testing → Task 3
- §9 versioning/release → noted out-of-scope; handled by `/release`

**Placeholder scan:** None — every step shows actual code or command. No "TBD".

**Type consistency:** `TlsVersionPolicy` enum variants (`Auto`/`Tls12Only`/`Tls13Only`) and serde rename strings (`auto`/`tls12_only`/`tls13_only`) appear consistently in Task 1, Task 2 match arm, Task 3 test helpers, Task 4 Tauri parse, Task 5 frontend payload.
