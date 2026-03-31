<script setup lang="ts">
import { inject, computed, ref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'
import type { ReceivedDataPointInfo } from '../types'

const { showAlert } = inject<{ showAlert: typeof ShowAlert }>(dialogKey)!
const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedPoints = inject<Ref<ReceivedDataPointInfo[]>>('selectedPoints')!

const hasSelection = computed(() => selectedPoints.value.length > 0)
const firstPoint = computed(() => selectedPoints.value[0] ?? null)

// Control command form
const cmdType = ref<'single' | 'double' | 'setpoint_float'>('single')
const cmdValue = ref('')
const cmdSelect = ref(false)
const cmdIoa = ref<number>(0)

// Auto-fill IOA from selection
const controlIoa = computed(() => {
  if (firstPoint.value) return firstPoint.value.ioa
  return cmdIoa.value
})

async function sendCommand() {
  if (!selectedConnectionId.value) return
  const ioa = controlIoa.value

  try {
    // Fetch common address for this connection
    const conns = await invoke<any[]>('list_connections')
    const conn = conns.find((c: any) => c.id === selectedConnectionId.value)
    const ca = conn?.common_address ?? 1

    await invoke('send_control_command', {
      request: {
        connection_id: selectedConnectionId.value,
        ioa: ioa,
        common_address: ca,
        command_type: cmdType.value,
        value: cmdValue.value,
        select: cmdSelect.value,
      }
    })
  } catch (e) {
    await showAlert(String(e))
  }
}

function cmdTypeLabel(t: string): string {
  const map: Record<string, string> = {
    single: 'C_SC_NA_1',
    double: 'C_DC_NA_1',
    setpoint_float: 'C_SE_NC_1',
  }
  return map[t] || t
}
</script>

<template>
  <div class="value-panel">
    <div class="panel-header">数据详情</div>

    <div v-if="!hasSelection" class="empty-state">
      选择数据点查看详情
    </div>

    <template v-else>
      <!-- Selected point details -->
      <div class="detail-section">
        <div class="section-title">选中数据点</div>
        <div v-for="point in selectedPoints" :key="point.ioa" class="detail-item">
          <div class="detail-row">
            <span class="detail-label">IOA</span>
            <span class="detail-value mono">{{ point.ioa }}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">类型</span>
            <span class="detail-value mono">{{ point.asdu_type }}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">分类</span>
            <span class="detail-value">{{ point.category }}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">值</span>
            <span class="detail-value mono">{{ point.value }}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">品质</span>
            <span :class="['detail-value', point.quality_iv ? 'text-red' : 'text-green']">
              {{ point.quality_iv ? 'IV (无效)' : 'OK (有效)' }}
            </span>
          </div>
          <div class="detail-row">
            <span class="detail-label">时间戳</span>
            <span class="detail-value mono">{{ point.timestamp ?? '无' }}</span>
          </div>
          <div v-if="selectedPoints.length > 1" class="detail-divider"></div>
        </div>
      </div>

      <!-- Control command section -->
      <div class="control-section">
        <div class="section-title">控制命令</div>

        <div class="control-form">
          <label class="form-label">
            IOA
            <input
              v-model.number="cmdIoa"
              class="form-input"
              type="number"
              min="0"
              :placeholder="String(controlIoa)"
              :value="controlIoa"
              @input="cmdIoa = Number(($event.target as HTMLInputElement).value)"
            />
          </label>

          <label class="form-label">
            命令类型
            <select v-model="cmdType" class="form-input">
              <option value="single">单点命令 (C_SC_NA_1)</option>
              <option value="double">双点命令 (C_DC_NA_1)</option>
              <option value="setpoint_float">浮点设定值 (C_SE_NC_1)</option>
            </select>
          </label>

          <label class="form-label">
            值
            <input
              v-model="cmdValue"
              class="form-input"
              type="text"
              :placeholder="cmdType === 'single' ? '0 或 1' : cmdType === 'double' ? '0-3' : '0.0'"
            />
          </label>

          <div class="toggle-row">
            <label class="toggle-label">
              <input type="checkbox" v-model="cmdSelect" class="toggle-checkbox" />
              <span>选择/执行</span>
            </label>
            <span class="toggle-hint">{{ cmdSelect ? '选择 (Select)' : '执行 (Execute)' }}</span>
          </div>

          <button
            class="btn btn-primary send-btn"
            :disabled="!selectedConnectionId"
            @click="sendCommand"
          >
            发送 {{ cmdTypeLabel(cmdType) }}
          </button>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.value-panel {
  padding: 0;
  font-size: 13px;
}

.panel-header {
  padding: 8px 12px;
  font-size: 11px;
  text-transform: uppercase;
  color: #6c7086;
  letter-spacing: 0.5px;
}

.empty-state {
  padding: 24px 12px;
  color: #6c7086;
  text-align: center;
  font-size: 12px;
}

.detail-section {
  border-bottom: 1px solid #313244;
  padding-bottom: 8px;
}

.section-title {
  padding: 6px 12px;
  font-size: 11px;
  color: #6c7086;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.detail-item {
  padding: 0 4px;
}

.detail-row {
  display: flex;
  justify-content: space-between;
  padding: 3px 12px;
}

.detail-label {
  color: #6c7086;
  font-size: 12px;
}

.detail-value {
  color: #cdd6f4;
  font-size: 12px;
  text-align: right;
}

.detail-value.mono {
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.text-green {
  color: #a6e3a1;
}

.text-red {
  color: #f38ba8;
}

.detail-divider {
  height: 1px;
  background: #313244;
  margin: 6px 12px;
}

.control-section {
  padding-bottom: 12px;
}

.control-form {
  padding: 0 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: #6c7086;
}

.form-input {
  padding: 5px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
}

.form-input:focus {
  outline: none;
  border-color: #89b4fa;
}

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 0;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #cdd6f4;
  cursor: pointer;
}

.toggle-checkbox {
  accent-color: #89b4fa;
}

.toggle-hint {
  font-size: 10px;
  color: #6c7086;
}

.send-btn {
  margin-top: 4px;
}

.btn {
  padding: 7px 16px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  text-align: center;
}

.btn-primary {
  background: #89b4fa;
  color: #1e1e2e;
  font-weight: 600;
}

.btn-primary:hover {
  background: #74c7ec;
}

.btn-primary:disabled {
  opacity: 0.4;
  cursor: default;
}
</style>
