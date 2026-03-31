<script setup lang="ts">
import { ref, watch, inject } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'

const { showAlert } = inject<{ showAlert: typeof ShowAlert }>(dialogKey)!

interface Props {
  visible: boolean
  serverId: string
  commonAddress: number
}

const props = defineProps<Props>()
const emit = defineEmits<{
  close: []
  added: []
}>()

const ASDU_TYPES = [
  { value: 'MSpNa1', label: 'M_SP_NA_1 - 单点信息' },
  { value: 'MDpNa1', label: 'M_DP_NA_1 - 双点信息' },
  { value: 'MStNa1', label: 'M_ST_NA_1 - 步位置信息' },
  { value: 'MBoNa1', label: 'M_BO_NA_1 - 位串' },
  { value: 'MMeNa1', label: 'M_ME_NA_1 - 归一化测量值' },
  { value: 'MMeNb1', label: 'M_ME_NB_1 - 标度化测量值' },
  { value: 'MMeNc1', label: 'M_ME_NC_1 - 浮点测量值' },
  { value: 'MItNa1', label: 'M_IT_NA_1 - 累计量' },
]

const formIoa = ref<number | undefined>(undefined)
const formAsduType = ref('MSpNa1')
const formName = ref('')
const formComment = ref('')
const isSaving = ref(false)

watch(() => props.visible, (visible) => {
  if (visible) {
    formIoa.value = undefined
    formAsduType.value = 'MSpNa1'
    formName.value = ''
    formComment.value = ''
    isSaving.value = false
  }
})

async function handleConfirm() {
  if (formIoa.value === undefined || formIoa.value < 0) {
    await showAlert('请输入有效的 IOA (>= 0)')
    return
  }
  isSaving.value = true
  try {
    await invoke('add_data_point', {
      request: {
        server_id: props.serverId,
        common_address: props.commonAddress,
        ioa: formIoa.value,
        asdu_type: formAsduType.value,
        name: formName.value || null,
        comment: formComment.value || null,
      },
    })
    emit('added')
    emit('close')
  } catch (e) {
    await showAlert(String(e))
  } finally {
    isSaving.value = false
  }
}

function handleBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains('modal-backdrop')) {
    emit('close')
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click="handleBackdropClick">
      <div class="modal">
        <div class="modal-header">
          <span class="modal-title">添加数据点</span>
          <button class="btn-close" @click="$emit('close')">×</button>
        </div>

        <div class="modal-body">
          <div class="form-group">
            <label class="form-label">IOA (信息对象地址)</label>
            <input
              v-model.number="formIoa"
              type="number"
              class="form-input"
              min="0"
              placeholder="例如: 100"
              @keyup.enter="handleConfirm"
            />
          </div>

          <div class="form-group">
            <label class="form-label">ASDU 类型</label>
            <select v-model="formAsduType" class="form-select">
              <option v-for="t in ASDU_TYPES" :key="t.value" :value="t.value">
                {{ t.label }}
              </option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">名称 (可选)</label>
            <input v-model="formName" type="text" class="form-input" placeholder="可留空" />
          </div>

          <div class="form-group">
            <label class="form-label">备注 (可选)</label>
            <input v-model="formComment" type="text" class="form-input" placeholder="可留空" />
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn btn-secondary" @click="$emit('close')" :disabled="isSaving">取消</button>
          <button class="btn btn-primary" @click="handleConfirm" :disabled="isSaving">
            {{ isSaving ? '添加中...' : '确认' }}
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
  z-index: 2000;
}

.modal {
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 8px;
  width: 420px;
  max-width: 90vw;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid #313244;
}

.modal-title {
  font-size: 16px;
  font-weight: 600;
  color: #cdd6f4;
}

.btn-close {
  background: none;
  border: none;
  color: #6c7086;
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}

.btn-close:hover {
  color: #cdd6f4;
}

.modal-body {
  padding: 20px;
}

.form-group {
  margin-bottom: 16px;
}

.form-label {
  display: block;
  font-size: 13px;
  color: #6c7086;
  margin-bottom: 6px;
}

.form-input,
.form-select {
  width: 100%;
  padding: 8px 12px;
  background: #11111b;
  border: 1px solid #45475a;
  border-radius: 6px;
  color: #cdd6f4;
  font-size: 14px;
  box-sizing: border-box;
}

.form-input:focus,
.form-select:focus {
  outline: none;
  border-color: #89b4fa;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 16px 20px;
  border-top: 1px solid #313244;
}

.btn {
  padding: 8px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
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
  cursor: not-allowed;
}

.btn-secondary {
  background: #45475a;
  color: #cdd6f4;
}

.btn-secondary:hover {
  background: #585b70;
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
