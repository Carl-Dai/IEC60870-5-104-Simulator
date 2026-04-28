export const APP_NAME = 'IEC104 Slave'
export const REPO_URL = 'https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator'
export const RELEASES_URL = 'https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator/releases'

// Keep in sync with CHANGELOG.md — see `release` skill.
export const RELEASE_NOTES: string[] = [
  '新增: 应用内自动更新 — 启动时静默检查 GitHub Releases, 发现新版本弹窗提示, 用户确认后下载并自动重启 (ed25519 验签, 6 小时节流, "稍后" 24 小时不重提)',
  '改进: 修正 GitHub 仓库链接 (旧链接 IEC104Sim 已失效, 改为 IEC60870-5-104-Simulator)',
]
