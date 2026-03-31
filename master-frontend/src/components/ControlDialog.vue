<script setup lang="ts">
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface Props {
  visible: boolean
  connectionId: string | null
  commonAddress: number
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'sent'): void
}>()

const ioa = ref<number>(0)
const commandType = ref<'single' | 'double' | 'setpoint_float'>('single')
const value = ref('')
const selectMode = ref(false)
const errorMsg = ref('')
const sending = ref(false)

watch(() => props.visible, (v) => {
  if (v) {
    errorMsg.value = ''
    sending.value = false
  }
})

async function send() {
  if (!props.connectionId) return
  errorMsg.value = ''
  sending.value = true

  try {
    await invoke('send_control_command', {
      request: {
        connection_id: props.connectionId,
        ioa: ioa.value,
        common_address: props.commonAddress,
        command_type: commandType.value,
        value: value.value,
        select: selectMode.value,
      }
    })
    sending.value = false
    emit('sent')
    emit('close')
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
    <div v-if="visible" class="modal-backdrop" @click.self="emit('close')" @keydown="handleKeydown">
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
              <option value="single">单点命令 (C_SC_NA_1)</option>
              <option value="double">双点命令 (C_DC_NA_1)</option>
              <option value="setpoint_float">浮点设定值 (C_SE_NC_1)</option>
            </select>
          </label>

          <label class="form-label">
            值
            <input
              v-model="value"
              class="form-input"
              type="text"
              :placeholder="commandType === 'single' ? 'true / false / 0 / 1' : commandType === 'double' ? '0-3' : '0.0'"
            />
          </label>

          <div class="toggle-row">
            <label class="toggle-label">
              <input type="checkbox" v-model="selectMode" class="toggle-checkbox" />
              <span>选择模式 (Select)</span>
            </label>
            <span class="toggle-hint">{{ selectMode ? '选择 (S/E = Select)' : '执行 (S/E = Execute)' }}</span>
          </div>

          <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="emit('close')">取消</button>
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
  min-width: 380px;
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
