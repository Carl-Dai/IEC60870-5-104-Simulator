<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { CommandType, ControlResult } from '../types'

interface Props {
  visible: boolean
  connectionId: string | null
  commonAddress: number
  prefillIoa?: number | null
  prefillCommandType?: CommandType | null
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'sent'): void
}>()

const ioa = ref<number>(0)
const commandType = ref<CommandType>('single')
const selectMode = ref(false)
const errorMsg = ref('')
const sending = ref(false)
const lastResult = ref<ControlResult | null>(null)

// Value state per type
const singleValue = ref('true')
const doubleValue = ref('2')
const stepValue = ref('2')
const normalizedValue = ref('0.0')
const scaledValue = ref('0')
const floatValue = ref('0.0')

watch(() => props.visible, (v) => {
  if (v) {
    errorMsg.value = ''
    sending.value = false
    lastResult.value = null
    if (props.prefillIoa != null) ioa.value = props.prefillIoa
    if (props.prefillCommandType) commandType.value = props.prefillCommandType
  }
})

const currentValueStr = computed(() => {
  switch (commandType.value) {
    case 'single': return singleValue.value
    case 'double': return doubleValue.value
    case 'step': return stepValue.value
    case 'setpoint_normalized': return normalizedValue.value
    case 'setpoint_scaled': return scaledValue.value
    case 'setpoint_float': return floatValue.value
  }
})

const COMMAND_TYPES: { value: CommandType; label: string }[] = [
  { value: 'single', label: '单点命令 (C_SC_NA_1)' },
  { value: 'double', label: '双点命令 (C_DC_NA_1)' },
  { value: 'step', label: '步调节命令 (C_RC_NA_1)' },
  { value: 'setpoint_normalized', label: '归一化设定值 (C_SE_NA_1)' },
  { value: 'setpoint_scaled', label: '标度化设定值 (C_SE_NB_1)' },
  { value: 'setpoint_float', label: '浮点设定值 (C_SE_NC_1)' },
]

async function send() {
  if (!props.connectionId) return
  errorMsg.value = ''
  sending.value = true
  lastResult.value = null

  try {
    const result = await invoke<ControlResult>('send_control_command', {
      request: {
        connection_id: props.connectionId,
        ioa: ioa.value,
        common_address: props.commonAddress,
        command_type: commandType.value,
        value: currentValueStr.value,
        select: selectMode.value,
      }
    })
    lastResult.value = result
    sending.value = false
    emit('sent')
  } catch (e) {
    errorMsg.value = String(e)
    sending.value = false
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close')
  } else if (e.key === 'Enter') {
    send()
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @mousedown.self="emit('close')" @keydown="handleKeydown">
      <div class="modal-box">
        <div class="modal-title">发送控制命令</div>
        <div class="modal-body">
          <label class="form-label">
            IOA (信息对象地址)
            <input v-model.number="ioa" class="form-input" type="number" min="0" max="16777215" />
          </label>

          <label class="form-label">
            命令类型
            <select v-model="commandType" class="form-input">
              <option v-for="ct in COMMAND_TYPES" :key="ct.value" :value="ct.value">{{ ct.label }}</option>
            </select>
          </label>

          <!-- Single point: toggle -->
          <div v-if="commandType === 'single'" class="ctrl-buttons">
            <button :class="['ctrl-btn', { active: singleValue === 'false' }]" @click="singleValue = 'false'">分闸 OFF</button>
            <button :class="['ctrl-btn', { active: singleValue === 'true' }]" @click="singleValue = 'true'">合闸 ON</button>
          </div>

          <!-- Double point: 4 buttons -->
          <div v-else-if="commandType === 'double'" class="ctrl-buttons">
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '0' }]" @click="doubleValue = '0'">中间</button>
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '1' }]" @click="doubleValue = '1'">分</button>
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '2' }]" @click="doubleValue = '2'">合</button>
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '3' }]" @click="doubleValue = '3'">不确定</button>
          </div>

          <!-- Step: up/down -->
          <div v-else-if="commandType === 'step'" class="ctrl-buttons">
            <button :class="['ctrl-btn', { active: stepValue === '1' }]" @click="stepValue = '1'">&#9660; 降</button>
            <button :class="['ctrl-btn', { active: stepValue === '2' }]" @click="stepValue = '2'">&#9650; 升</button>
          </div>

          <!-- Normalized: slider + input -->
          <div v-else-if="commandType === 'setpoint_normalized'" class="slider-control">
            <div class="slider-row">
              <input type="range" class="slider-input" min="-1" max="1" step="0.001" v-model="normalizedValue" />
              <input type="number" class="number-sm" min="-1" max="1" step="0.001" v-model="normalizedValue" />
            </div>
          </div>

          <!-- Scaled: integer input -->
          <label v-else-if="commandType === 'setpoint_scaled'" class="form-label">
            值 (-32768 ~ 32767)
            <input v-model="scaledValue" class="form-input" type="number" min="-32768" max="32767" step="1" />
          </label>

          <!-- Float: number input -->
          <label v-else-if="commandType === 'setpoint_float'" class="form-label">
            值
            <input v-model="floatValue" class="form-input" type="number" step="0.1" />
          </label>

          <div class="toggle-row">
            <label class="toggle-label">
              <input type="checkbox" v-model="selectMode" class="toggle-checkbox" />
              <span>选择-执行 (SbO)</span>
            </label>
            <span class="toggle-hint">{{ selectMode ? '自动两步' : '直接执行' }}</span>
          </div>

          <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>

          <div v-if="lastResult" class="result-indicator result-ok">
            <span class="result-steps">
              <span v-for="(step, i) in lastResult.steps" :key="i" class="step-dot" :title="step.action">&#9679;</span>
            </span>
            <span class="result-text">OK {{ lastResult.duration_ms }}ms</span>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="emit('close')">关闭</button>
          <button class="btn btn-primary" :disabled="sending" @click="send">
            {{ sending ? '发送中...' : '发送' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-box {
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 8px;
  padding: 20px;
  min-width: 400px;
  max-width: 90vw;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: #cdd6f4;
  margin-bottom: 16px;
}

.modal-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}

.form-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: #6c7086;
}

.form-input {
  padding: 6px 10px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 13px;
}

.form-input:focus {
  outline: none;
  border-color: #89b4fa;
}

.ctrl-buttons {
  display: flex;
  gap: 6px;
}

.ctrl-btn {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #45475a;
  border-radius: 6px;
  background: #313244;
  color: #cdd6f4;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
}

.ctrl-btn:hover {
  background: #45475a;
}

.ctrl-btn.active {
  background: #89b4fa;
  color: #1e1e2e;
  border-color: #89b4fa;
  font-weight: 600;
}

.ctrl-btn-sm {
  padding: 6px 8px;
  font-size: 11px;
}

.slider-control {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.slider-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.slider-input {
  flex: 1;
  accent-color: #89b4fa;
}

.number-sm {
  width: 80px;
  padding: 4px 6px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.number-sm:focus {
  outline: none;
  border-color: #89b4fa;
}

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 0;
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

.error-msg {
  padding: 8px 10px;
  background: rgba(243, 139, 168, 0.15);
  border: 1px solid #f38ba8;
  border-radius: 4px;
  color: #f38ba8;
  font-size: 12px;
  word-break: break-word;
}

.result-indicator {
  padding: 6px 8px;
  border-radius: 4px;
  font-size: 11px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.result-ok {
  background: rgba(166, 227, 161, 0.15);
  border: 1px solid rgba(166, 227, 161, 0.3);
  color: #a6e3a1;
}

.result-steps {
  display: flex;
  gap: 4px;
  font-size: 8px;
}

.step-dot {
  color: #a6e3a1;
}

.result-text {
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.btn {
  padding: 7px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
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
  opacity: 0.5;
  cursor: default;
}

.btn-secondary {
  background: #45475a;
  color: #cdd6f4;
}

.btn-secondary:hover {
  background: #585b70;
}
</style>
