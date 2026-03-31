<script setup lang="ts">
import { ref, inject, watch, onUnmounted, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert, showConfirm as ShowConfirm, showPrompt as ShowPrompt } from '../composables/useDialog'

const selectedServerId = inject<Ref<string | null>>('selectedServerId')!
const selectedServerState = inject<Ref<string>>('selectedServerState')!
const selectedCA = inject<Ref<number | null>>('selectedCA')!
const refreshTree = inject<() => void>('refreshTree')!
const refreshData = inject<() => void>('refreshData')!
const { showAlert, showConfirm, showPrompt } = inject<{
  showAlert: typeof ShowAlert
  showConfirm: typeof ShowConfirm
  showPrompt: typeof ShowPrompt
}>(dialogKey)!

// --- New Server Modal ---
const showNewServerModal = ref(false)
const newServerPort = ref('2404')
const newServerInitMode = ref('zero')
const newServerUseTls = ref(false)
const newServerCertFile = ref('')
const newServerKeyFile = ref('')
const newServerCaFile = ref('')
const newServerRequireClientCert = ref(false)

function openNewServerModal() {
  newServerPort.value = '2404'
  newServerInitMode.value = 'zero'
  newServerUseTls.value = false
  newServerCertFile.value = ''
  newServerKeyFile.value = ''
  newServerCaFile.value = ''
  newServerRequireClientCert.value = false
  showNewServerModal.value = true
}

async function submitNewServer() {
  const port = Number(newServerPort.value)
  if (!port || port < 1 || port > 65535) {
    await showAlert('请输入有效的端口号 (1-65535)')
    return
  }
  showNewServerModal.value = false
  try {
    await invoke('create_server', {
      request: {
        port,
        init_mode: newServerInitMode.value,
        use_tls: newServerUseTls.value || undefined,
        cert_file: newServerCertFile.value || undefined,
        key_file: newServerKeyFile.value || undefined,
        ca_file: newServerCaFile.value || undefined,
        require_client_cert: newServerRequireClientCert.value || undefined,
      },
    })
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

// --- Start / Stop ---
async function startServer() {
  if (!selectedServerId.value) return
  try {
    await invoke('start_server', { id: selectedServerId.value })
    selectedServerState.value = 'Running'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function stopServer() {
  if (!selectedServerId.value) return
  try {
    await invoke('stop_server', { id: selectedServerId.value })
    selectedServerState.value = 'Stopped'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

// --- Add Station ---
async function addStation() {
  if (!selectedServerId.value) return
  const caStr = await showPrompt('输入公共地址 (CA)', '1')
  if (caStr === null) return
  const ca = Number(caStr)
  if (isNaN(ca) || ca < 1 || ca > 65534) {
    await showAlert('请输入有效的公共地址 (1-65534)')
    return
  }
  const name = await showPrompt('输入站名', `站 ${ca}`)
  if (name === null) return
  try {
    await invoke('add_station', {
      request: {
        server_id: selectedServerId.value,
        common_address: ca,
        name: name || `站 ${ca}`,
      },
    })
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

// --- Add Data Point ---
async function addDataPoint() {
  if (!selectedServerId.value || selectedCA.value === null) return
  const ioaStr = await showPrompt('输入信息对象地址 (IOA)', '1')
  if (ioaStr === null) return
  const ioa = Number(ioaStr)
  if (isNaN(ioa) || ioa < 0) {
    await showAlert('请输入有效的 IOA')
    return
  }
  try {
    await invoke('add_data_point', {
      request: {
        server_id: selectedServerId.value,
        common_address: selectedCA.value,
        ioa,
        asdu_type: 'MSpNa1',
      },
    })
    refreshData()
  } catch (e) {
    await showAlert(String(e))
  }
}

// --- Random Mutation ---
const mutationActive = ref(false)
const mutationRate = ref(1000)
let mutationTimer: number | null = null

function toggleMutation() {
  if (mutationActive.value) {
    stopMutation()
  } else {
    startMutation()
  }
}

function startMutation() {
  if (!selectedServerId.value || selectedCA.value === null) return
  mutationActive.value = true
  scheduleMutation()
}

function stopMutation() {
  mutationActive.value = false
  if (mutationTimer !== null) {
    clearTimeout(mutationTimer)
    mutationTimer = null
  }
}

function scheduleMutation() {
  if (!mutationActive.value) return
  mutationTimer = window.setTimeout(async () => {
    if (!mutationActive.value || !selectedServerId.value || selectedCA.value === null) {
      stopMutation()
      return
    }
    try {
      await invoke('random_mutate_data_points', {
        request: {
          server_id: selectedServerId.value,
          common_address: selectedCA.value,
        },
      })
      refreshData()
    } catch (e) {
      console.error('mutation failed:', e)
    }
    scheduleMutation()
  }, mutationRate.value)
}

watch([selectedServerId, selectedCA], () => {
  if (mutationActive.value) stopMutation()
})

onUnmounted(() => {
  if (mutationTimer !== null) clearTimeout(mutationTimer)
})
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openNewServerModal" title="新建服务器">
        <span class="toolbar-icon">+</span>
        <span class="toolbar-label">新建服务器</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button
        class="toolbar-btn btn-start"
        @click="startServer"
        :disabled="!selectedServerId || selectedServerState === 'Running'"
        title="启动服务器"
      >
        <span class="toolbar-label">启动</span>
      </button>
      <button
        class="toolbar-btn btn-stop"
        @click="stopServer"
        :disabled="!selectedServerId || selectedServerState === 'Stopped'"
        title="停止服务器"
      >
        <span class="toolbar-label">停止</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button
        class="toolbar-btn"
        @click="addStation"
        :disabled="!selectedServerId"
        title="添加站"
      >
        <span class="toolbar-label">添加站</span>
      </button>
      <button
        class="toolbar-btn"
        @click="addDataPoint"
        :disabled="!selectedServerId || selectedCA === null"
        title="添加数据点"
      >
        <span class="toolbar-label">添加数据点</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group mutation-group">
      <button
        :class="['toolbar-btn', { 'btn-mutation-active': mutationActive }]"
        @click="toggleMutation"
        :disabled="!selectedServerId || selectedCA === null"
        title="随机变化"
      >
        <span class="toolbar-label">{{ mutationActive ? '停止变化' : '随机变化' }}</span>
      </button>
      <input
        type="range"
        class="rate-slider"
        min="100"
        max="5000"
        step="100"
        v-model.number="mutationRate"
        title="变化间隔 (ms)"
      />
      <span class="rate-label">{{ mutationRate }}ms</span>
    </div>
    <div class="toolbar-title">IEC 104 Slave</div>
  </div>

  <!-- New Server Modal -->
  <Teleport to="body">
    <div v-if="showNewServerModal" class="modal-overlay" @click.self="showNewServerModal = false">
      <div class="modal-box">
        <div class="modal-title">新建服务器</div>
        <div class="modal-field">
          <label>端口号</label>
          <input
            v-model="newServerPort"
            type="number"
            min="1"
            max="65535"
            @keyup.enter="submitNewServer"
          />
        </div>
        <div class="modal-field">
          <label>初始值</label>
          <div class="radio-group">
            <label class="radio-label">
              <input type="radio" v-model="newServerInitMode" value="zero" /> 全零
            </label>
            <label class="radio-label">
              <input type="radio" v-model="newServerInitMode" value="random" /> 随机
            </label>
          </div>
        </div>
        <div class="modal-field">
          <label class="checkbox-label">
            <input type="checkbox" v-model="newServerUseTls" /> 启用 TLS
          </label>
        </div>
        <template v-if="newServerUseTls">
          <div class="modal-field">
            <label>服务器证书文件 (PEM)</label>
            <input
              v-model="newServerCertFile"
              type="text"
              placeholder="/path/to/server.crt"
            />
          </div>
          <div class="modal-field">
            <label>服务器密钥文件 (PEM)</label>
            <input
              v-model="newServerKeyFile"
              type="text"
              placeholder="/path/to/server.key"
            />
          </div>
          <div class="modal-field">
            <label>CA 证书文件 (PEM, 可选)</label>
            <input
              v-model="newServerCaFile"
              type="text"
              placeholder="/path/to/ca.crt"
            />
          </div>
          <div class="modal-field">
            <label class="checkbox-label">
              <input type="checkbox" v-model="newServerRequireClientCert" /> 要求客户端证书 (mTLS)
            </label>
          </div>
        </template>
        <div class="modal-actions">
          <button class="modal-btn cancel" @click="showNewServerModal = false">取消</button>
          <button class="modal-btn confirm" @click="submitNewServer">确定</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 8px;
  gap: 6px;
  user-select: none;
  font-size: 13px;
}

.toolbar-group {
  display: flex;
  gap: 2px;
}

.toolbar-divider {
  width: 1px;
  height: 24px;
  background: #313244;
  margin: 0 4px;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border: none;
  background: #313244;
  color: #cdd6f4;
  cursor: pointer;
  border-radius: 4px;
  font-size: 13px;
  white-space: nowrap;
}

.toolbar-btn:hover:not(:disabled) {
  background: #45475a;
}

.toolbar-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.toolbar-btn.btn-start:not(:disabled) {
  color: #a6e3a1;
}

.toolbar-btn.btn-stop:not(:disabled) {
  color: #fab387;
}

.toolbar-icon {
  font-weight: bold;
  font-size: 14px;
}

.toolbar-btn.btn-mutation-active {
  background: #a6e3a1;
  color: #1e1e2e;
  font-weight: 600;
}

.toolbar-btn.btn-mutation-active:hover {
  background: #94e2d5;
}

.mutation-group {
  align-items: center;
}

.rate-slider {
  width: 80px;
  height: 4px;
  accent-color: #89b4fa;
  cursor: pointer;
}

.rate-label {
  font-size: 10px;
  color: #6c7086;
  min-width: 42px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.toolbar-title {
  margin-left: auto;
  font-size: 12px;
  color: #6c7086;
  padding-right: 8px;
}

/* Modal styles */
.modal-overlay {
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
  min-width: 300px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: #cdd6f4;
  margin-bottom: 16px;
}

.modal-field {
  margin-bottom: 14px;
}

.modal-field label {
  display: block;
  font-size: 12px;
  color: #a6adc8;
  margin-bottom: 6px;
}

.modal-field input[type="number"],
.modal-field input[type="text"] {
  width: 100%;
  padding: 6px 10px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 13px;
  outline: none;
  box-sizing: border-box;
}

.modal-field input[type="number"]:focus,
.modal-field input[type="text"]:focus {
  border-color: #89b4fa;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: #cdd6f4;
  cursor: pointer;
}

.checkbox-label input[type="checkbox"] {
  accent-color: #89b4fa;
}

.radio-group {
  display: flex;
  gap: 16px;
}

.radio-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: #cdd6f4;
  cursor: pointer;
}

.radio-label input[type="radio"] {
  accent-color: #89b4fa;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 18px;
}

.modal-btn {
  padding: 6px 16px;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
}

.modal-btn.cancel {
  background: #313244;
  color: #a6adc8;
}

.modal-btn.cancel:hover {
  background: #45475a;
}

.modal-btn.confirm {
  background: #89b4fa;
  color: #1e1e2e;
  font-weight: 600;
}

.modal-btn.confirm:hover {
  background: #74c7ec;
}
</style>
