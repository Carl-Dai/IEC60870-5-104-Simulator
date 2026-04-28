<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from '../i18n'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  version: string
  notes: string
}>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'snooze'): void
}>()

const downloading = ref(false)
const progress = ref(0)
const error = ref<string | null>(null)
let unlisten: UnlistenFn | null = null

async function install() {
  error.value = null
  downloading.value = true
  progress.value = 0
  unlisten = await listen<number>('update-progress', (e) => {
    progress.value = e.payload
  })
  try {
    await invoke('install_update')
  } catch (e: any) {
    error.value = String(e)
    downloading.value = false
  } finally {
    if (unlisten) { unlisten(); unlisten = null }
  }
}

function later() {
  emit('snooze')
  emit('close')
}
</script>

<template>
  <div v-if="visible" class="update-overlay">
    <div class="update-dialog">
      <h3>{{ t('update.available') }}</h3>
      <p>{{ t('update.newVersion', { version }) }}</p>
      <details open>
        <summary>{{ t('update.changelog') }}</summary>
        <pre class="notes">{{ notes }}</pre>
      </details>

      <div v-if="downloading" class="progress">
        {{ t('update.downloading', { pct: progress }) }}
        <progress :value="progress" max="100"></progress>
      </div>

      <div v-if="error" class="error">
        <strong>{{ t('update.failedTitle') }}</strong>
        <pre>{{ error }}</pre>
      </div>

      <div class="actions">
        <button v-if="!downloading && !error" @click="later">{{ t('update.later') }}</button>
        <button v-if="!downloading && !error" @click="install">{{ t('update.installNow') }}</button>
        <button v-if="error" @click="install">{{ t('update.retry') }}</button>
        <button v-if="error" @click="$emit('close')">{{ t('update.close') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.update-overlay {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.45);
  display: flex; align-items: center; justify-content: center;
  z-index: 9999;
}
.update-dialog {
  background: var(--surface, #fff);
  color: var(--text, #222);
  padding: 20px 24px;
  border-radius: 8px;
  min-width: 420px; max-width: 560px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.25);
}
.notes { white-space: pre-wrap; max-height: 240px; overflow: auto; font-size: 13px; }
.progress { margin-top: 12px; }
.progress progress { width: 100%; }
.error { margin-top: 12px; color: #b00020; }
.error pre { white-space: pre-wrap; font-size: 12px; }
.actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 16px; }
</style>
