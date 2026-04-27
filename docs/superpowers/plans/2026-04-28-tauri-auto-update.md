# Tauri 自动更新实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 接入 `tauri-plugin-updater`，让 `iec104sim-app` 与 `iec104master-app` 在启动时自动从 GitHub Releases 检测、下载并安装新版本，弹窗提示用户。

**Architecture:** 客户端定时（启动时）拉取同一 release 上的 `latest-slave.json` / `latest-master.json`；ed25519 验签后下载替换并重启。CI 在 build 后追加 `publish-manifest` job 生成两个 manifest 并上传。

**Tech Stack:** Tauri 2.10 · tauri-plugin-updater 2 · tauri-plugin-process 2 · tauri-plugin-store 2 · Vue 3 · GitHub Actions · Node 20 (manifest 脚本) · vitest。

**对应 spec：** `docs/superpowers/specs/2026-04-28-tauri-auto-update-design.md`

---

## 文件结构总览

| 路径 | 操作 | 责任 |
|---|---|---|
| `crates/iec104sim-app/Cargo.toml` | 修改 | 加入 updater / process / store 依赖 |
| `crates/iec104sim-app/tauri.conf.json` | 修改 | `plugins.updater` 配置（endpoint + pubkey） |
| `crates/iec104sim-app/src/lib.rs` | 修改 | 注册插件 + 注册新命令 |
| `crates/iec104sim-app/src/update.rs` | 创建 | 纯函数 `should_check` / `is_snoozed` + commands `check_for_update` / `install_update` / `snooze_update` |
| `crates/iec104master-app/...` 同上 | 同上 | master 端镜像 |
| `frontend/src/composables/useUpdater.ts` | 创建 | 封装 invoke + 事件监听，给 App.vue 调用 |
| `frontend/src/components/UpdateDialog.vue` | 创建 | UI 弹窗 |
| `frontend/src/i18n/locales/zh-CN.ts` / `en-US.ts` | 修改 | 新增 `update.*` 键 |
| `frontend/src/i18n/types.ts` | 修改 (若有 update 需在 DictShape) | 类型同步 |
| `frontend/src/App.vue` | 修改 | `onMounted` 触发更新检查、挂载 dialog |
| `master-frontend/src/...` 同上 | 同上 | master 端镜像 |
| `scripts/gen-update-manifest.mjs` | 创建 | 从 GitHub release assets 生成两个 manifest |
| `scripts/gen-update-manifest.test.mjs` | 创建 | vitest 单元测试 |
| `package.json`（根） | 创建或修改 | 给脚本配 `vitest`，避免污染 frontend 包 |
| `.github/workflows/release.yml` | 修改 | 注入签名 env + 新增 `publish-manifest` job |
| `CHANGELOG.md` / `README.md` | 修改 | 说明从该版本起支持自动更新 |

---

## Task 1: 一次性密钥与凭据准备（人工操作）

**这个 task 不写代码，是发版前置条件。**Subagent 不要尝试自动化它，应在执行其他任务前要求用户完成并确认。

**Files:** 无（密钥生成在用户本地，secrets 在 GitHub UI 配置）

- [ ] **Step 1: 安装 Tauri CLI**

```bash
cargo install tauri-cli --version "^2"
```

预期：`cargo tauri --version` 输出 `tauri-cli 2.x.x`。

- [ ] **Step 2: 生成 ed25519 密钥对**

```bash
mkdir -p ~/.tauri
cargo tauri signer generate -w ~/.tauri/iec104.key
```

按提示输入密码（**记录到密码管理器**）。
预期输出：私钥文件 `~/.tauri/iec104.key` + 公钥（base64 字符串），形如 `dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...`。

- [ ] **Step 3: 配置 GitHub repository secrets**

到 https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator/settings/secrets/actions ，新增：

- `TAURI_SIGNING_PRIVATE_KEY` = `cat ~/.tauri/iec104.key` 的完整内容（包括首尾行）
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = Step 2 设的密码

- [ ] **Step 4: 把公钥复制到一个临时位置**

```bash
cat ~/.tauri/iec104.key.pub
```

把这串 base64 公钥贴到一个 scratch 文件备用 —— Task 3 要写进两个 `tauri.conf.json`。

---

## Task 2: 添加 Rust 端依赖

**Files:**
- Modify: `crates/iec104sim-app/Cargo.toml`
- Modify: `crates/iec104master-app/Cargo.toml`

- [ ] **Step 1: 在 slave Cargo.toml 的 `[dependencies]` 末尾追加**

```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
tauri-plugin-store = "2"
```

- [ ] **Step 2: 在 master Cargo.toml 同样追加（同三行）**

- [ ] **Step 3: 验证编译能解析依赖**

```bash
cargo check -p iec104sim-app -p iec104master-app
```

预期：通过。如出现版本冲突，把版本固定为 `2.x` 与 `tauri = "2.10.3"` 兼容的最新 minor（在 https://crates.io/crates/tauri-plugin-updater 查看）。

- [ ] **Step 4: Commit**

```bash
git add crates/iec104sim-app/Cargo.toml crates/iec104master-app/Cargo.toml Cargo.lock
git commit -m "chore(deps): add tauri updater/process/store plugins"
```

---

## Task 3: 配置 tauri.conf.json updater 段（双 app）

**Files:**
- Modify: `crates/iec104sim-app/tauri.conf.json`
- Modify: `crates/iec104master-app/tauri.conf.json`

替换 `<PUBKEY>` 为 Task 1 Step 4 拷贝出的公钥。

- [ ] **Step 1: slave 配置**

在 `crates/iec104sim-app/tauri.conf.json` 顶层（与 `app`、`bundle` 同级）添加 `plugins` 段；如已存在 `plugins` 则在其下追加 `updater`：

```json
"plugins": {
  "updater": {
    "endpoints": [
      "https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator/releases/latest/download/latest-slave.json"
    ],
    "pubkey": "<PUBKEY>"
  }
}
```

- [ ] **Step 2: master 配置**

`crates/iec104master-app/tauri.conf.json` 同样新增 `plugins` 段，但 endpoint 末尾文件名改成 `latest-master.json`，pubkey 同上。

- [ ] **Step 3: 验证 JSON schema**

```bash
cargo check -p iec104sim-app -p iec104master-app
```

预期：`tauri-build` 在 build script 中校验 `tauri.conf.json` 通过；如失败按报错调整字段层级。

- [ ] **Step 4: Commit**

```bash
git add crates/iec104sim-app/tauri.conf.json crates/iec104master-app/tauri.conf.json
git commit -m "feat(updater): configure github releases endpoint for both apps"
```

---

## Task 4: 写 update.rs 纯函数 + 失败测试

**Files:**
- Create: `crates/iec104sim-app/src/update.rs`
- Create: `crates/iec104sim-app/tests/update_helpers.rs`

先用 TDD 写两个纯函数：节流判断 + snooze 判断。

- [ ] **Step 1: 写失败的集成测试 `crates/iec104sim-app/tests/update_helpers.rs`**

```rust
use chrono::{DateTime, Duration, Utc};
use iec104sim_app_lib::update::{is_snoozed, should_check};

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

#[test]
fn should_check_when_no_prior_check() {
    assert!(should_check(None, ts("2026-04-28T10:00:00Z"), Duration::hours(6)));
}

#[test]
fn should_skip_within_throttle_window() {
    let last = ts("2026-04-28T08:00:00Z");
    let now = ts("2026-04-28T10:00:00Z");
    assert!(!should_check(Some(last), now, Duration::hours(6)));
}

#[test]
fn should_check_after_throttle_window() {
    let last = ts("2026-04-28T03:00:00Z");
    let now = ts("2026-04-28T10:00:00Z");
    assert!(should_check(Some(last), now, Duration::hours(6)));
}

#[test]
fn snoozed_when_same_version_within_window() {
    assert!(is_snoozed(
        Some("1.0.9"),
        Some(ts("2026-04-29T00:00:00Z")),
        "1.0.9",
        ts("2026-04-28T10:00:00Z"),
    ));
}

#[test]
fn not_snoozed_after_window_expires() {
    assert!(!is_snoozed(
        Some("1.0.9"),
        Some(ts("2026-04-28T09:00:00Z")),
        "1.0.9",
        ts("2026-04-28T10:00:00Z"),
    ));
}

#[test]
fn not_snoozed_for_different_version() {
    assert!(!is_snoozed(
        Some("1.0.9"),
        Some(ts("2026-04-29T00:00:00Z")),
        "1.0.10",
        ts("2026-04-28T10:00:00Z"),
    ));
}
```

- [ ] **Step 2: 运行测试确认失败（模块还不存在）**

```bash
cargo test -p iec104sim-app --test update_helpers
```

预期：编译失败，`unresolved import iec104sim_app_lib::update`。

- [ ] **Step 3: 实现 `crates/iec104sim-app/src/update.rs`**

```rust
use chrono::{DateTime, Duration, Utc};

pub fn should_check(
    last_check: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    throttle: Duration,
) -> bool {
    match last_check {
        None => true,
        Some(last) => now - last >= throttle,
    }
}

pub fn is_snoozed(
    snoozed_version: Option<&str>,
    snoozed_until: Option<DateTime<Utc>>,
    remote_version: &str,
    now: DateTime<Utc>,
) -> bool {
    match (snoozed_version, snoozed_until) {
        (Some(v), Some(until)) => v == remote_version && now < until,
        _ => false,
    }
}
```

- [ ] **Step 4: 在 `crates/iec104sim-app/src/lib.rs` 顶部加模块声明**

```rust
mod commands;
mod state;
pub mod update;   // 新增
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cargo test -p iec104sim-app --test update_helpers
```

预期：6 个 test 全部 PASS。

- [ ] **Step 6: 镜像到 master**

把 `update.rs` 复制到 `crates/iec104master-app/src/update.rs`（**内容完全一致**），同样在 `crates/iec104master-app/src/lib.rs` 加 `pub mod update;`。把 `tests/update_helpers.rs` 复制到 `crates/iec104master-app/tests/update_helpers.rs`，把 `iec104sim_app_lib::update` 改为 `iec104master_app_lib::update`。

```bash
cargo test -p iec104master-app --test update_helpers
```

预期：6 个 test 全部 PASS。

- [ ] **Step 7: Commit**

```bash
git add crates/iec104sim-app/src/{update.rs,lib.rs} crates/iec104sim-app/tests/update_helpers.rs \
        crates/iec104master-app/src/{update.rs,lib.rs} crates/iec104master-app/tests/update_helpers.rs
git commit -m "feat(updater): add throttle and snooze pure helpers"
```

---

## Task 5: Slave 端 update commands + 插件注册

**Files:**
- Modify: `crates/iec104sim-app/src/update.rs`
- Modify: `crates/iec104sim-app/src/lib.rs`

- [ ] **Step 1: 在 `update.rs` 末尾追加 commands**

```rust
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tauri_plugin_updater::UpdaterExt;

const STORE_FILE: &str = "update_state.json";
const KEY_LAST_CHECK: &str = "last_check_at";
const KEY_SNOOZED_VER: &str = "snoozed_version";
const KEY_SNOOZED_UNTIL: &str = "snoozed_until";
const THROTTLE_HOURS: i64 = 6;
const SNOOZE_HOURS: i64 = 24;

#[derive(Serialize, Clone)]
pub struct UpdateMeta {
    pub version: String,
    pub notes: String,
    pub pub_date: Option<String>,
}

fn read_str(app: &AppHandle, key: &str) -> Option<String> {
    let store = app.store(STORE_FILE).ok()?;
    store.get(key).and_then(|v| v.as_str().map(String::from))
}

fn write_str(app: &AppHandle, key: &str, value: &str) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(key, serde_json::Value::String(value.to_string()));
        let _ = store.save();
    }
}

fn parse_ts(s: Option<String>) -> Option<DateTime<Utc>> {
    s.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateMeta>, String> {
    let now = Utc::now();
    let last = parse_ts(read_str(&app, KEY_LAST_CHECK));
    if !should_check(last, now, Duration::hours(THROTTLE_HOURS)) {
        return Ok(None);
    }
    write_str(&app, KEY_LAST_CHECK, &now.to_rfc3339());

    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = match updater.check().await {
        Ok(u) => u,
        Err(e) => {
            log::warn!("update check failed: {e}");
            return Ok(None);
        }
    };
    let Some(update) = update else { return Ok(None) };

    let snoozed_v = read_str(&app, KEY_SNOOZED_VER);
    let snoozed_u = parse_ts(read_str(&app, KEY_SNOOZED_UNTIL));
    if is_snoozed(snoozed_v.as_deref(), snoozed_u, &update.version, now) {
        return Ok(None);
    }

    Ok(Some(UpdateMeta {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        pub_date: update.date.map(|d| d.to_string()),
    }))
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no update available".to_string())?;

    let mut downloaded: u64 = 0;
    let app_clone = app.clone();
    update
        .download_and_install(
            move |chunk_len, content_len| {
                downloaded += chunk_len as u64;
                if let Some(total) = content_len {
                    let pct = (downloaded as f64 / total as f64 * 100.0).round() as u32;
                    let _ = app_clone.emit("update-progress", pct);
                }
            },
            || {
                log::info!("update downloaded, installing");
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    app.restart();
}

#[tauri::command]
pub fn snooze_update(app: AppHandle, version: String) -> Result<(), String> {
    let until = Utc::now() + Duration::hours(SNOOZE_HOURS);
    write_str(&app, KEY_SNOOZED_VER, &version);
    write_str(&app, KEY_SNOOZED_UNTIL, &until.to_rfc3339());
    Ok(())
}
```

- [ ] **Step 2: 在 `lib.rs` 注册插件与命令**

把 `lib.rs` 的 `tauri::Builder::default()` 链改为：

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_store::Builder::new().build())
    .manage(AppState::new())
    .invoke_handler(tauri::generate_handler![
        // ...所有现有命令保留...
        update::check_for_update,
        update::install_update,
        update::snooze_update,
    ])
    .setup(|app| { /* 现有 setup 不动 */ Ok(()) })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

- [ ] **Step 3: 编译**

```bash
cargo build -p iec104sim-app
```

预期：通过。如报 `chrono` 未引入或 `Emitter` 未在 `tauri::Manager` trait 中，按提示补 `use`。

- [ ] **Step 4: Commit**

```bash
git add crates/iec104sim-app/src/{update.rs,lib.rs}
git commit -m "feat(slave): wire updater commands + tauri plugins"
```

---

## Task 6: Master 端镜像 commands + 插件注册

**Files:**
- Modify: `crates/iec104master-app/src/update.rs`
- Modify: `crates/iec104master-app/src/lib.rs`

- [ ] **Step 1: 把 Task 5 Step 1 的全部代码复制到 `crates/iec104master-app/src/update.rs` 的末尾**

（与 slave 完全一致 —— 两个 app 行为对称）

- [ ] **Step 2: 在 `crates/iec104master-app/src/lib.rs` 同样注册插件与三个命令**

参照 Task 5 Step 2 的写法，挂在 master 现有的 `Builder::default()` 链上。

- [ ] **Step 3: 编译**

```bash
cargo build -p iec104master-app
```

预期：通过。

- [ ] **Step 4: Commit**

```bash
git add crates/iec104master-app/src/{update.rs,lib.rs}
git commit -m "feat(master): wire updater commands + tauri plugins"
```

---

## Task 7: 新增 i18n 键（双前端）

**Files:**
- Modify: `frontend/src/i18n/locales/zh-CN.ts`
- Modify: `frontend/src/i18n/locales/en-US.ts`
- Modify: `master-frontend/src/i18n/locales/zh-CN.ts`
- Modify: `master-frontend/src/i18n/locales/en-US.ts`

四个文件**各自独立**修改，键名一致。

- [ ] **Step 1: 在每个 `zh-CN.ts` 的 `DictShape` 类型与导出对象中追加 `update` 段**

类型部分（在 `DictShape = { ... }` 内的合适位置）：

```ts
update: {
  available: string
  newVersion: string
  changelog: string
  installNow: string
  later: string
  downloading: string
  failedTitle: string
  retry: string
  close: string
}
```

导出对象部分（在文件末尾的 `const zhCN: DictShape = { ... }` 内追加）：

```ts
update: {
  available: '检测到新版本',
  newVersion: '新版本 v{version} 可用',
  changelog: '更新说明',
  installNow: '立即更新',
  later: '稍后',
  downloading: '正在下载 {pct}%',
  failedTitle: '更新失败',
  retry: '重试',
  close: '关闭',
}
```

- [ ] **Step 2: 在每个 `en-US.ts` 追加对应英文**

```ts
update: {
  available: 'Update available',
  newVersion: 'Version v{version} is available',
  changelog: 'Release notes',
  installNow: 'Install now',
  later: 'Later',
  downloading: 'Downloading {pct}%',
  failedTitle: 'Update failed',
  retry: 'Retry',
  close: 'Close',
}
```

- [ ] **Step 3: 类型检查**

```bash
cd frontend && npm run build
cd ../master-frontend && npm run build
```

预期：两个工程 vue-tsc 通过。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/i18n/locales/*.ts master-frontend/src/i18n/locales/*.ts
git commit -m "feat(i18n): add update.* keys to both frontends"
```

---

## Task 8: 实现 UpdateDialog.vue（slave）

**Files:**
- Create: `frontend/src/components/UpdateDialog.vue`

- [ ] **Step 1: 写组件**

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { t } from '../i18n'

const props = defineProps<{
  visible: boolean
  version: string
  notes: string
}>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'snooze'): void
}>()

const downloading = ref(false)
const progress = ref(0)
const error = ref<string | null>(null)
let unlisten: UnlistenFn | null = null

async function install() {
  error.value = null
  downloading.value = true
  progress.value = 0
  unlisten = await listen<number>('update-progress', (e) => {
    progress.value = e.payload
  })
  try {
    await invoke('install_update')
  } catch (e: any) {
    error.value = String(e)
    downloading.value = false
  } finally {
    if (unlisten) { unlisten(); unlisten = null }
  }
}

function later() {
  emit('snooze')
  emit('close')
}
</script>

<template>
  <div v-if="visible" class="update-overlay">
    <div class="update-dialog">
      <h3>{{ t('update.available') }}</h3>
      <p>{{ t('update.newVersion', { version }) }}</p>
      <details open>
        <summary>{{ t('update.changelog') }}</summary>
        <pre class="notes">{{ notes }}</pre>
      </details>

      <div v-if="downloading" class="progress">
        {{ t('update.downloading', { pct: progress }) }}
        <progress :value="progress" max="100"></progress>
      </div>

      <div v-if="error" class="error">
        <strong>{{ t('update.failedTitle') }}</strong>
        <pre>{{ error }}</pre>
      </div>

      <div class="actions">
        <button v-if="!downloading && !error" @click="later">{{ t('update.later') }}</button>
        <button v-if="!downloading && !error" @click="install">{{ t('update.installNow') }}</button>
        <button v-if="error" @click="install">{{ t('update.retry') }}</button>
        <button v-if="error" @click="$emit('close')">{{ t('update.close') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.update-overlay {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.45);
  display: flex; align-items: center; justify-content: center;
  z-index: 9999;
}
.update-dialog {
  background: var(--surface, #fff);
  color: var(--text, #222);
  padding: 20px 24px;
  border-radius: 8px;
  min-width: 420px; max-width: 560px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.25);
}
.notes { white-space: pre-wrap; max-height: 240px; overflow: auto; font-size: 13px; }
.progress { margin-top: 12px; }
.progress progress { width: 100%; }
.error { margin-top: 12px; color: #b00020; }
.error pre { white-space: pre-wrap; font-size: 12px; }
.actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 16px; }
</style>
```

- [ ] **Step 2: 类型检查**

```bash
cd frontend && npm run build
```

预期：通过。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/UpdateDialog.vue
git commit -m "feat(slave): add UpdateDialog component"
```

---

## Task 9: 实现 UpdateDialog.vue（master）

**Files:**
- Create: `master-frontend/src/components/UpdateDialog.vue`

- [ ] **Step 1: 把 Task 8 Step 1 的整段代码复制到 `master-frontend/src/components/UpdateDialog.vue`**

（路径相对引用 `../i18n` 在两端结构相同，可直接复用）

- [ ] **Step 2: 类型检查**

```bash
cd master-frontend && npm run build
```

预期：通过。

- [ ] **Step 3: Commit**

```bash
git add master-frontend/src/components/UpdateDialog.vue
git commit -m "feat(master): add UpdateDialog component"
```

---

## Task 10: App.vue 启动钩子（slave）

**Files:**
- Modify: `frontend/src/App.vue`

- [ ] **Step 1: 在 `<script setup>` 顶部 import 段追加**

```ts
import { onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import UpdateDialog from './components/UpdateDialog.vue'
```

（如果 `onMounted` 已 import 则跳过）

- [ ] **Step 2: 在 `<script setup>` 末尾追加更新逻辑**

```ts
const updateMeta = ref<{ version: string; notes: string } | null>(null)
const updateVisible = ref(false)

async function checkUpdate() {
  try {
    const meta = await invoke<{ version: string; notes: string } | null>('check_for_update')
    if (meta) {
      updateMeta.value = meta
      updateVisible.value = true
    }
  } catch (e) {
    console.warn('update check failed', e)
  }
}

onMounted(() => {
  setTimeout(checkUpdate, 2000)
})

function snoozeUpdate() {
  if (updateMeta.value) {
    invoke('snooze_update', { version: updateMeta.value.version }).catch(() => {})
  }
}
```

- [ ] **Step 3: 在 `<template>` 末尾（紧贴 `</template>` 前，与现有 `AppDialog` 同层）插入**

```vue
<UpdateDialog
  :visible="updateVisible"
  :version="updateMeta?.version ?? ''"
  :notes="updateMeta?.notes ?? ''"
  @close="updateVisible = false"
  @snooze="snoozeUpdate"
/>
```

- [ ] **Step 4: 类型检查**

```bash
cd frontend && npm run build
```

预期：通过。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/App.vue
git commit -m "feat(slave): trigger update check on app start"
```

---

## Task 11: App.vue 启动钩子（master）

**Files:**
- Modify: `master-frontend/src/App.vue`

- [ ] **Step 1: 重复 Task 10 的 Step 1–3**，但路径在 `master-frontend/`。`onMounted` 在 master App.vue 中已 import，可省略。

- [ ] **Step 2: 类型检查**

```bash
cd master-frontend && npm run build
```

预期：通过。

- [ ] **Step 3: Commit**

```bash
git add master-frontend/src/App.vue
git commit -m "feat(master): trigger update check on app start"
```

---

## Task 12: 写 manifest 生成脚本 + 单元测试

**Files:**
- Create: `scripts/gen-update-manifest.mjs`
- Create: `scripts/gen-update-manifest.test.mjs`
- Create: `scripts/package.json`（让 vitest 在 scripts 子目录跑）

- [ ] **Step 1: 创建 `scripts/package.json`**

```json
{
  "name": "iec104sim-scripts",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run"
  },
  "devDependencies": {
    "vitest": "^4.1.5"
  }
}
```

```bash
cd scripts && npm install
```

- [ ] **Step 2: 写失败的测试 `scripts/gen-update-manifest.test.mjs`**

```js
import { describe, it, expect } from 'vitest'
import { groupAssetsByRole, extractChangelogSection } from './gen-update-manifest.mjs'

const sample = [
  { name: 'IEC104Slave_1.0.9_aarch64.app.tar.gz', browser_download_url: 'u1' },
  { name: 'IEC104Slave_1.0.9_aarch64.app.tar.gz.sig', browser_download_url: 'u1s' },
  { name: 'IEC104Slave_1.0.9_x64-setup.nsis.zip', browser_download_url: 'u2' },
  { name: 'IEC104Slave_1.0.9_x64-setup.nsis.zip.sig', browser_download_url: 'u2s' },
  { name: 'IEC104Master_1.0.9_amd64.AppImage.tar.gz', browser_download_url: 'u3' },
  { name: 'IEC104Master_1.0.9_amd64.AppImage.tar.gz.sig', browser_download_url: 'u3s' },
]

describe('groupAssetsByRole', () => {
  it('separates slave and master assets', () => {
    const { slave, master } = groupAssetsByRole(sample)
    expect(slave['darwin-aarch64'].url).toBe('u1')
    expect(slave['darwin-aarch64'].sigUrl).toBe('u1s')
    expect(slave['windows-x86_64'].url).toBe('u2')
    expect(master['linux-x86_64'].url).toBe('u3')
  })
})

describe('extractChangelogSection', () => {
  const md = `# Changelog\n\n## 1.0.9\n- foo\n- bar\n\n## 1.0.8\n- old\n`
  it('extracts the section for the given version', () => {
    expect(extractChangelogSection(md, '1.0.9')).toBe('- foo\n- bar')
  })
  it('returns empty string when version not found', () => {
    expect(extractChangelogSection(md, '9.9.9')).toBe('')
  })
})
```

- [ ] **Step 3: 运行测试确认失败**

```bash
cd scripts && npx vitest run
```

预期：模块未实现，全部 FAIL。

- [ ] **Step 4: 实现 `scripts/gen-update-manifest.mjs`**

```js
#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const REPO = 'kelsoprotein-lab/IEC60870-5-104-Simulator'

const PLATFORM_PATTERNS = [
  { key: 'darwin-aarch64', re: /aarch64\.app\.tar\.gz$/ },
  { key: 'darwin-x86_64',  re: /x64\.app\.tar\.gz$/ },
  { key: 'windows-x86_64', re: /x64-setup\.nsis\.zip$/ },
  { key: 'linux-x86_64',   re: /amd64\.AppImage\.tar\.gz$/ },
]

export function groupAssetsByRole(assets) {
  const groups = { slave: {}, master: {} }
  const sigByUrl = new Map()
  for (const a of assets) {
    if (a.name.endsWith('.sig')) sigByUrl.set(a.name.slice(0, -4), a.browser_download_url)
  }
  for (const a of assets) {
    if (a.name.endsWith('.sig')) continue
    const role = a.name.startsWith('IEC104Slave_') ? 'slave'
              : a.name.startsWith('IEC104Master_') ? 'master' : null
    if (!role) continue
    const plat = PLATFORM_PATTERNS.find((p) => p.re.test(a.name))
    if (!plat) continue
    groups[role][plat.key] = {
      url: a.browser_download_url,
      sigUrl: sigByUrl.get(a.name),
    }
  }
  return groups
}

export function extractChangelogSection(md, version) {
  const lines = md.split('\n')
  const startRe = new RegExp(`^##\\s+${version.replace(/\./g, '\\.')}\\b`)
  let inSection = false
  const out = []
  for (const line of lines) {
    if (startRe.test(line)) { inSection = true; continue }
    if (inSection && /^##\s+/.test(line)) break
    if (inSection) out.push(line)
  }
  return out.join('\n').trim()
}

async function fetchSigContent(url) {
  // GitHub asset URL is public (release is not draft); plain fetch ok
  const res = await fetch(url)
  if (!res.ok) throw new Error(`fetch sig failed: ${url} ${res.status}`)
  return (await res.text()).trim()
}

async function main() {
  const tag = process.argv[2]
  if (!tag) { console.error('usage: gen-update-manifest.mjs <tag>'); process.exit(1) }
  const version = tag.replace(/^v/, '')

  const json = execFileSync('gh', ['api', `repos/${REPO}/releases/tags/${tag}`], { encoding: 'utf8' })
  const release = JSON.parse(json)
  const grouped = groupAssetsByRole(release.assets)

  const changelogPath = resolve(process.cwd(), 'CHANGELOG.md')
  const notes = extractChangelogSection(readFileSync(changelogPath, 'utf8'), version)
  const pubDate = release.published_at

  for (const role of ['slave', 'master']) {
    const platforms = {}
    for (const [key, val] of Object.entries(grouped[role])) {
      const sig = await fetchSigContent(val.sigUrl)
      platforms[key] = { signature: sig, url: val.url }
    }
    if (Object.keys(platforms).length === 0) {
      throw new Error(`no platforms found for role ${role}`)
    }
    const manifest = { version, notes, pub_date: pubDate, platforms }
    const out = resolve(process.cwd(), `latest-${role}.json`)
    writeFileSync(out, JSON.stringify(manifest, null, 2))
    console.log(`wrote ${out}`)
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => { console.error(e); process.exit(1) })
}
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd scripts && npx vitest run
```

预期：全部 PASS。

- [ ] **Step 6: Commit**

```bash
git add scripts/
git commit -m "feat(ci): add latest-{role}.json generator script with tests"
```

---

## Task 13: 修改 release.yml 注入签名 + publish-manifest job

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 在两个 `tauri-apps/tauri-action@v0` step 的 `env:` 段追加签名变量**

slave job 与 master job 都要改。原来的 env：

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

改为：

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

- [ ] **Step 2: 在文件末尾追加新 job**

```yaml
  publish-manifest:
    needs: [build-slave, build-master]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - name: Install script deps
        working-directory: scripts
        run: npm install --no-audit --no-fund
      - name: Generate manifests
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: node scripts/gen-update-manifest.mjs ${{ github.ref_name }}
      - name: Upload manifests to release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: gh release upload ${{ github.ref_name }} latest-slave.json latest-master.json --clobber
```

- [ ] **Step 3: yamllint（如本地有）或 GitHub Actions 在线校验**

```bash
# 简单校验语法
python3 -c "import yaml,sys;yaml.safe_load(open('.github/workflows/release.yml'))"
```

预期：无异常输出。

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): sign updater artifacts and publish manifests"
```

---

## Task 14: 文档与 release notes

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `README.md` 与 `README_CN.md`

- [ ] **Step 1: `CHANGELOG.md` 顶部追加段落**

```markdown
## 1.0.9 (待发布)

- feat: 新增应用内自动更新（启动时静默检查 GitHub Releases，发现新版本时弹窗提示）
- 已发布的旧版本（含 1.0.8）需要手动升级一次到 1.0.9，之后将自动收到后续更新
```

- [ ] **Step 2: 在两个 README 末尾"Release / 发版"章节加一小节**

```markdown
### Auto-update

Starting from v1.0.9 the apps check GitHub Releases on startup and prompt the user to install
new versions. Users on v1.0.8 or earlier need to upgrade manually one time.
```

中文版同样追加一段中文说明。

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md README.md README_CN.md
git commit -m "docs: announce in-app auto update from 1.0.9"
```

---

## Task 15: 端到端验证（人工）

**这一步不写代码，是发版前的最终验证。**

- [ ] **Step 1: 本地 mock 验证（可选但推荐）**

```bash
# 临时复制 1.0.8 安装包并伪造一个 latest-slave.json，version 写 1.0.9，url 指回 1.0.8 安装包
# 在 scripts 目录下随手起一个静态服务
cd /tmp && python3 -m http.server 8000
```

临时改 `crates/iec104sim-app/tauri.conf.json` 的 endpoint 指向 `http://localhost:8000/latest-slave.json`，本地运行已安装的 v1.0.8 → 触发弹窗 → 验证下载、验签、重启。**验完务必 git checkout 还原 endpoint。**

- [ ] **Step 2: 推一个 RC tag，让真实 CI 跑一遍**

```bash
git tag v1.0.9-rc.1
git push origin v1.0.9-rc.1
```

到 GitHub Actions 看 `release.yml` 完整通过；到 release 页确认 `latest-slave.json` 与 `latest-master.json` 都已上传。

- [ ] **Step 3: 三平台老版本测试**

在 macOS/Windows/Linux 三台机器（或 VM）上各装一份 v1.0.8 → 启动 → 等待弹窗 → 一键更新 → 重启后版本号变成 v1.0.9-rc.1。

- [ ] **Step 4: 失败兜底验证**

断网启动 → 不应弹任何错误；前端 console 仅有一条 `update check failed` warning。

- [ ] **Step 5: 验证通过后打正式 tag**

```bash
git tag v1.0.9
git push origin v1.0.9
```

---

## 执行注意事项

- **Task 1 必须由用户完成**：subagent 不要尝试自动生成密钥或写 secrets。其他任务在 Task 1 完成后才能产出可发版的成果（之前可以并行准备代码）。
- **每个 Task 完成后跑相应的验证命令**（`cargo check`、`npm run build`、`vitest`），不要堆到最后。
- **Slave 与 master 改动严格对称**：每次 slave 改完后立刻镜像到 master，避免漂移。
- **不要把 endpoint 改回 localhost 后忘记还原**（Task 15 Step 1 的常见坑）。
