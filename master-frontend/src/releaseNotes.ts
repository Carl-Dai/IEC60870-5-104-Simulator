export const APP_NAME = 'IEC104 Master'
export const REPO_URL = 'https://github.com/kelsoprotein-lab/IEC104Sim'
export const RELEASES_URL = 'https://github.com/kelsoprotein-lab/IEC104Sim/releases'

// Keep in sync with CHANGELOG.md — see `release` skill.
export const RELEASE_NOTES: string[] = [
  '修复: 从站端口关闭后,主站连接状态不更新且无法重连',
  '修复: 在输入框内拖选文字到弹窗外松开鼠标会误关弹窗',
  '改进: 使用 watch 通道统一主站状态变更通知,简化核心实现',
]
