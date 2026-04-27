export const APP_NAME = 'IEC104 Master'
export const REPO_URL = 'https://github.com/kelsoprotein-lab/IEC104Sim'
export const RELEASES_URL = 'https://github.com/kelsoprotein-lab/IEC104Sim/releases'

// Keep in sync with CHANGELOG.md — see `release` skill.
export const RELEASE_NOTES: string[] = [
  '新增: UI 支持中英文运行时切换 (工具栏 中/EN 按钮一键切换,首次启动跟随系统语言并持久化)',
  '新增: 通信日志详情列改前端字典渲染,切换语言时已显示日志立即随之更新',
  '新增: 日志 CSV 导出改前端实现,跟随当前 UI 语言',
  '改进: 核心库 LogEntry 增加可选 detail_event 字段 (向后兼容)',
]
