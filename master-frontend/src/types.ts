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
}

export interface LogEntry {
  timestamp: string
  direction: string
  frame_label: { [key: string]: string } | string
  detail: string
  raw_bytes: number[] | null
}
