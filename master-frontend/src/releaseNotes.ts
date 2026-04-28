export const APP_NAME = 'IEC104 Master'
export const REPO_URL = 'https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator'
export const RELEASES_URL = 'https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator/releases'

// Keep in sync with CHANGELOG.md — see `release` skill.
export const RELEASE_NOTES: string[] = [
  '新增: 应用内自动更新 — 启动时静默检查 GitHub Releases, 发现新版本弹窗提示, 用户确认后下载并自动重启 (ed25519 验签, 6 小时节流, "稍后" 24 小时不重提)',
  '新增: 一个连接支持多个公共地址 (CA) — "新建连接" 对话框输入逗号分隔的列表 (例如 1, 2, 3), 自动 GI / 时钟同步 / 累计量召唤按 CA 列表循环, 连接树显示 CA:1,2,3',
  '改进: 修正 GitHub 仓库链接 (旧链接 IEC104Sim 已失效, 改为 IEC60870-5-104-Simulator)',
  '已知限制: 多 CA 场景下右键单点控制命令仅发到连接的第一个 CA (数据点未携带 CA 信息)',
]
