export const APP_NAME = 'IEC104 Master'
export const REPO_URL = 'https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator'
export const RELEASES_URL = 'https://github.com/kelsoprotein-lab/IEC60870-5-104-Simulator/releases'

// Keep in sync with CHANGELOG.md — see `release` skill.
export const RELEASE_NOTES: string[] = [
  '新增: 连接列表右键菜单加 "编辑连接" — 复用新建对话框, 字段全部预填, 保存时先删旧再建新; 编辑前需断开',
  '改进: README 顶部加多 CA 与通信日志截图章节, 更直观地展示主站能力',
  '修复: macOS 给 .app 加 ad-hoc 签名, 修 v1.1.1 及之前下载后被判定为 "已损坏" 的问题',
  '修复: CI publish-manifest 启动时 release 还没对 API 可见导致挂掉, 现在 6×5s 自动重试 Not Found',
]
