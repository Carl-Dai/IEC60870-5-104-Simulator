export interface ConnectionInfo {
  id: string
  target_address: string
  port: number
  common_address: number
  state: string
  use_tls: boolean
}

export interface ReceivedDataPointInfo {
  ioa: number
  asdu_type: string
  category: string
  value: string
  quality_iv: boolean
  timestamp: string | null
  update_seq: number
}

export interface IncrementalDataResponse {
  seq: number
  total_count: number
  points: ReceivedDataPointInfo[]
}

export interface LogEntry {
  timestamp: string
  direction: string
  frame_label: { [key: string]: string } | string
  detail: string
  raw_bytes: number[] | null
}

export type CommandType = 'single' | 'double' | 'step' | 'setpoint_normalized' | 'setpoint_scaled' | 'setpoint_float'

export interface ControlStep {
  action: string
  timestamp: string
}

export interface ControlResult {
  steps: ControlStep[]
  duration_ms: number
}

export type WidgetType = 'toggle' | 'button_group' | 'step_buttons' | 'slider' | 'number_input'

export interface ControlOption {
  label: string
  value: string
}

export interface ControlConfig {
  commandType: CommandType
  label: string
  widget: WidgetType
  options?: ControlOption[]
  min?: number
  max?: number
  step?: number
}

const CONTROL_CONFIG_MAP: Record<string, ControlConfig | null> = {
  '单点 (SP)': {
    commandType: 'single',
    label: '单点命令 C_SC_NA_1',
    widget: 'toggle',
    options: [
      { label: '分闸 OFF', value: 'false' },
      { label: '合闸 ON', value: 'true' },
    ],
  },
  '双点 (DP)': {
    commandType: 'double',
    label: '双点命令 C_DC_NA_1',
    widget: 'button_group',
    options: [
      { label: '中间', value: '0' },
      { label: '分', value: '1' },
      { label: '合', value: '2' },
      { label: '不确定', value: '3' },
    ],
  },
  '步位置 (ST)': {
    commandType: 'step',
    label: '步调节命令 C_RC_NA_1',
    widget: 'step_buttons',
    options: [
      { label: '降', value: '1' },
      { label: '升', value: '2' },
    ],
  },
  '归一化 (ME_NA)': {
    commandType: 'setpoint_normalized',
    label: '归一化设定值 C_SE_NA_1',
    widget: 'slider',
    min: -1.0,
    max: 1.0,
    step: 0.001,
  },
  '标度化 (ME_NB)': {
    commandType: 'setpoint_scaled',
    label: '标度化设定值 C_SE_NB_1',
    widget: 'number_input',
    min: -32768,
    max: 32767,
    step: 1,
  },
  '浮点 (ME_NC)': {
    commandType: 'setpoint_float',
    label: '浮点设定值 C_SE_NC_1',
    widget: 'number_input',
    step: 0.1,
  },
  '位串 (BO)': null,
  '累计量 (IT)': null,
}

export function getControlConfig(category: string): ControlConfig | null {
  return CONTROL_CONFIG_MAP[category] ?? null
}
