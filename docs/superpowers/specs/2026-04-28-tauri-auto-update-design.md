# Tauri 自动更新（推送式升级）设计

- 日期：2026-04-28
- 状态：设计已确认，待实施
- 适用应用：`iec104sim-app`（Slave）、`iec104master-app`（Master）

## 1. 背景与目标

当前发版流程为 push tag → GitHub Actions 通过 `tauri-action` 构建多平台安装包并上传 GitHub Releases。**用户须手动到 release 页下载安装**，老版本不会被告知有新版本。

目标：让两个 Tauri 应用在启动后**自动检测新版本**，弹窗提示用户、下载安装、重启应用。整体仍依赖 GitHub Releases 作为分发源，**不引入任何新基建**。

非目标：

- 服务端主动推送（push）。Tauri updater 本质是客户端定时拉取，本设计沿用此模型；用户体感为"推送"。
- 增量/差分升级。统一全包替换。
- 强制升级。允许用户"稍后"延后。

## 2. 关键决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 更新源 | GitHub Releases（A） | 零新基建，复用既有 CI 产物 |
| 触发时机 | 仅启动时检查（A） | 工业仿真器使用频率不高，无需轮询 |
| 签名方案 | 本地生成密钥 + GitHub Secrets，私钥带密码（A）| Tauri 强制签名；密钥不入仓库 |
| 更新 UI | 自定义 Vue 弹窗（B） | 与现有 i18n / 设计语言一致 |
| 多 app 清单组织 | 同一 release 挂 `latest-slave.json` 和 `latest-master.json`（A） | 两 app 解耦，CI 增量最小 |

## 3. 总体架构

```
                ┌────────────────────────────┐
                │   GitHub Releases (vX.Y.Z) │
                │  ├─ latest-slave.json      │  endpoint
                │  ├─ latest-master.json     │  endpoint
                │  ├─ *.dmg / *.app.tar.gz   │  安装包 + .sig
                │  ├─ *.msi / *.exe + .sig   │
                │  └─ *.AppImage + .sig      │
                └─────────────▲──────────────┘
                              │ 上传
            ┌─────────────────┴────────────────┐
            │ GitHub Actions release.yml       │
            │  build-slave   build-master      │
            │      └────────┬───────┘          │
            │               ▼                  │
            │        publish-manifest          │
            │   (生成并上传两个 latest-*.json) │
            └──────────────────────────────────┘

App 启动:
  Vue App.vue ──┐
                ▼
            invoke('check_for_update')
                │
                ▼
       tauri-plugin-updater 拉 latest-{role}.json
                │
       验签 / 版本比较 / 节流
                │
   ┌────────────┴────────────┐
   ▼                         ▼
 有新版本                 无 / 失败
   │                         │
   ▼                         └─ 静默写 log
 emit('update-available', meta)
   │
   ▼
 UpdateDialog.vue 显示 changelog
   │  用户点"立即更新"
   ▼
 invoke('install_update') → 下载（progress 事件）→ 验签 → 安装 → relaunch
```

## 4. 组件

### 4.1 后端（Rust）

每个 app 各自接入：

- `tauri-plugin-updater`：拉清单、下载、验签、安装。
- `tauri-plugin-process`：安装后调用 `app.restart()`。

Tauri commands（两个 app 各一份相同实现）：

| 命令 | 输入 | 输出 | 说明 |
|---|---|---|---|
| `check_for_update` | 无 | `Option<UpdateMeta>` | 返回 `{ version, notes, pub_date }` 或 `None` |
| `install_update` | 无 | `()` + 多次 `update-progress` 事件 | 失败回 `Err(String)` |

**节流**：使用 `tauri-plugin-store`（或本地 JSON）记录 `last_check_at`；6 小时内 `check_for_update` 直接返回 `None`。

**跳过**：用户点"稍后" → 写入 `snoozed_version` 与 `snoozed_until = now + 24h`，下次启动若 `latest.version == snoozed_version && now < snoozed_until` 则不弹窗。

### 4.2 前端（Vue）

新增组件 `UpdateDialog.vue`，分别放入 `frontend/src/components/` 与 `master-frontend/src/components/`（两份独立维护，避免引入共享子包；后续如有公共组件需求再抽）。

| Prop / state | 含义 |
|---|---|
| `visible` | 是否显示 |
| `version` | 新版本号 |
| `notes` | Markdown 字符串，组件内用 `marked` 或现有库渲染（master-frontend 已有 markdown 渲染能力，参照复用） |
| `progress` | 0–100，下载中显示进度条 |
| `error` | 安装失败的错误信息 |

按钮：

- **立即更新** → `invoke('install_update')`，进入下载状态；失败展示 `error` 并提供"重试 / 关闭"。
- **稍后** → 关闭弹窗，写 `snoozed_*`。

i18n：复用现有键体系，新增 `update.title`、`update.available`、`update.now`、`update.later`、`update.downloading`、`update.failed` 等键，中英双语。

入口：`App.vue` `onMounted` 后延迟 2 秒调用 `check_for_update`。仅启动时检查，不提供 UI 内的手动触发按钮（已与用户确认）。

### 4.3 配置文件

`crates/iec104sim-app/tauri.conf.json` 与 `crates/iec104master-app/tauri.conf.json` 各自加：

```json
"plugins": {
  "updater": {
    "endpoints": [
      "https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator/releases/latest/download/latest-slave.json"
    ],
    "pubkey": "<base64 ed25519 公钥>"
  }
}
```

master 应用替换为 `latest-master.json`。两个 app 共用同一对密钥，故 `pubkey` 相同。

### 4.4 CI（`.github/workflows/release.yml`）

**改动 A**：现有两个 build job 中的 `tauri-action` step 增补 env：

```yaml
TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

`tauri-action` 检测到密钥后自动产出 `*.sig` 并上传到 release。

**改动 B**：新增 job `publish-manifest`：

```yaml
publish-manifest:
  needs: [build-slave, build-master]
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with: { node-version: 20 }
    - name: Generate manifests
      env:
        GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      run: node scripts/gen-update-manifest.mjs ${{ github.ref_name }}
    - name: Upload manifests
      env:
        GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      run: gh release upload ${{ github.ref_name }} latest-slave.json latest-master.json --clobber
```

**`scripts/gen-update-manifest.mjs`** 职责：

1. `gh api repos/kelsoprotein-lab/IEC60870-5-104-Simulator/releases/tags/<tag>` 拉 release assets。
2. 按文件名前缀（`IEC104Slave_*` vs `IEC104Master_*`）和平台后缀（`.app.tar.gz`/`.nsis.zip`/`.AppImage.tar.gz`）分组。
3. 读对应 `*.sig` 文件内容（assets 中的 sig 通过 `gh release download` 取得）作为 `signature` 字段。
4. `version` 取 `<tag>` 去掉前缀 `v`。
5. `notes` 从 `CHANGELOG.md` 解析当前版本对应段落（按 `## 1.0.9` 这类标题切分）。
6. 输出 `latest-slave.json`、`latest-master.json` 到 workspace 根目录。

如有任一平台 sig 缺失，脚本以非零退出码失败，阻断 release。

## 5. 数据契约

### 5.1 `latest-{role}.json`

```json
{
  "version": "1.0.9",
  "notes": "## 1.0.9\n- 修复 …",
  "pub_date": "2026-04-28T10:00:00Z",
  "platforms": {
    "darwin-aarch64": { "signature": "<sig>", "url": "<asset url>" },
    "darwin-x86_64":  { "signature": "<sig>", "url": "<asset url>" },
    "windows-x86_64": { "signature": "<sig>", "url": "<asset url>" },
    "linux-x86_64":   { "signature": "<sig>", "url": "<asset url>" }
  }
}
```

字段名是 Tauri updater 的强制 schema，不可改。

### 5.2 本地状态（`tauri-plugin-store`）

```json
{
  "update": {
    "last_check_at": "2026-04-28T10:00:00Z",
    "snoozed_version": "1.0.9",
    "snoozed_until":   "2026-04-29T10:00:00Z"
  }
}
```

## 6. 错误与容错

| 场景 | 行为 |
|---|---|
| 网络不可达 / json 404 / 解析失败 | 静默写 log，不弹任何提示，不计入节流（下次启动仍会试） |
| 版本比较：本地 ≥ 远端 | 静默 |
| 在节流窗口内 | 静默 |
| 用户在 24h 跳过窗口内 | 静默 |
| 下载中网络断 | dialog 显示错误信息 + 重试按钮 |
| 验签失败 | dialog 显示"安装包校验失败，请重新启动应用稍后再试"，写 log |
| 安装阶段失败（权限/磁盘等）| 同上，提示用户手动到 release 页下载 |
| 私钥丢失 | 重新生成密钥对，更新 `pubkey` 发新版；CHANGELOG 注明老用户需手动升级一次 |

## 7. 测试策略

| 层级 | 做法 |
|---|---|
| 本地集成（手动）| 安装 v1.0.8 → `python -m http.server` 托管伪造的 `latest-slave.json`（version 改高、url 仍指向 1.0.8 安装包以便快速试） → 临时把 `tauri.conf.json` 的 endpoint 指向 localhost → 验证：弹窗、下载进度、验签、重启 |
| CI 端到端（一次性）| 合并后打一个 `v1.0.9-rc.1` 预发版 tag → 验证：(a) 两个 manifest 正确生成、(b) 三平台老版本 v1.0.8 能拉到并升级 |
| 单元（vitest）| `gen-update-manifest.mjs` 中的纯函数（按 asset 名分组、CHANGELOG 段落抽取）写测试；mock `gh api` 输出 |
| 回归 | 后续每次发版的 `publish-manifest` job 跑通即过；客户端验签失败会有日志 |

## 8. 实施任务清单（供 writing-plans 细化）

1. 本地生成 ed25519 密钥对，配置 GitHub Secrets。
2. 两个 `tauri.conf.json` 加 `plugins.updater` 段。
3. 两个 app `Cargo.toml` / `tauri.conf.json` 加 `tauri-plugin-updater`、`tauri-plugin-process` 依赖与初始化。
4. Rust 端实现 `check_for_update` / `install_update` 命令与节流/snooze 逻辑。
5. 前端实现 `UpdateDialog.vue`（× 2）、i18n 键、`App.vue` 启动钩子。
6. 新增 `scripts/gen-update-manifest.mjs` + 单元测试。
7. 修改 `.github/workflows/release.yml`：注入签名 env、新增 `publish-manifest` job。
8. 文档：更新 `README` / `CHANGELOG`，注明从该版本起支持自动更新；老用户需手动升级一次以上车。
9. 端到端验证：发 `v1.0.9-rc.1` 走完整链路。

## 9. 风险与开放点

- **第一次上车成本**：当前已发布的 v1.0.8 用户拿不到自动更新，必须手动升一次到首个支持 updater 的版本。需在 release notes 中显著说明。
- **GitHub 在国内的连通性**：偶有不稳。本设计不解决，依赖现状；若未来有镜像需求，可在 endpoints 数组里追加镜像 URL（Tauri updater 支持多 endpoint 顺序回退）。
- **Linux 包格式**：当前 CI 输出 AppImage，updater 对 AppImage 的覆盖安装支持成熟；若未来加 deb/rpm 需评估 updater 兼容性。
- **macOS notarization**：当前似未配置公证（建议另行确认）。如未公证，updater 替换的 .app 在某些 macOS 版本会被 Gatekeeper 拦下。本设计假设公证状态与现状一致；如需引入公证，单列任务。
