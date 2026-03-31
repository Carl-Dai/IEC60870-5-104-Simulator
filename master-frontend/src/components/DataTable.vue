<script setup lang="ts">
import { ref, inject, watch, onMounted, onUnmounted, computed, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ReceivedDataPointInfo } from '../types'

const emit = defineEmits<{
  (e: 'point-select', points: ReceivedDataPointInfo[]): void
}>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedCategory = inject<Ref<string | null>>('selectedCategory')!
const dataRefreshKey = inject<Ref<number>>('dataRefreshKey')!

const allPoints = ref<ReceivedDataPointInfo[]>([])
const selectedIndices = ref<Set<number>>(new Set())
const lastClickedIndex = ref<number>(-1)
const searchFilter = ref('')

// Track previous values to detect changes for flash animation
const previousValues = ref<Map<number, string>>(new Map())
const changedIoas = ref<Set<number>>(new Set())
let changeTimers: Map<number, number> = new Map()

let pollTimer: number | null = null

async function fetchData() {
  if (!selectedConnectionId.value) return
  try {
    const data = await invoke<ReceivedDataPointInfo[]>('get_received_data', {
      id: selectedConnectionId.value,
    })

    // Detect changes
    const newChanged = new Set<number>()
    for (const point of data) {
      const prev = previousValues.value.get(point.ioa)
      if (prev !== undefined && prev !== point.value) {
        newChanged.add(point.ioa)
        // Clear previous timer for this IOA
        const existingTimer = changeTimers.get(point.ioa)
        if (existingTimer) clearTimeout(existingTimer)
        // Set timer to remove flash after 2 seconds
        const timer = window.setTimeout(() => {
          changedIoas.value.delete(point.ioa)
          changeTimers.delete(point.ioa)
        }, 2000)
        changeTimers.set(point.ioa, timer)
      }
    }
    // Merge new changes
    for (const ioa of newChanged) {
      changedIoas.value.add(ioa)
    }
    // Update previous values map
    const newPrev = new Map<number, string>()
    for (const point of data) {
      newPrev.set(point.ioa, point.value)
    }
    previousValues.value = newPrev

    allPoints.value = data
  } catch (_e) { /* ignore */ }
}

function startPollTimer() {
  stopPollTimer()
  pollTimer = window.setInterval(fetchData, 2000)
}

function stopPollTimer() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

onMounted(() => {
  if (selectedConnectionId.value) {
    fetchData()
    startPollTimer()
  }
})

onUnmounted(() => {
  stopPollTimer()
  // Clear all change timers
  for (const timer of changeTimers.values()) {
    clearTimeout(timer)
  }
  changeTimers.clear()
})

watch([selectedConnectionId], () => {
  selectedIndices.value.clear()
  allPoints.value = []
  previousValues.value.clear()
  changedIoas.value.clear()
  emit('point-select', [])
  stopPollTimer()
  if (!selectedConnectionId.value) return
  fetchData()
  startPollTimer()
})

watch(dataRefreshKey, () => {
  fetchData()
})

// Filter by category
const filteredPoints = computed(() => {
  let points = allPoints.value
  if (selectedCategory.value) {
    points = points.filter(p => p.category === selectedCategory.value)
  }
  if (searchFilter.value) {
    const q = searchFilter.value.toLowerCase()
    points = points.filter(p =>
      p.ioa.toString().includes(q) ||
      p.asdu_type.toLowerCase().includes(q) ||
      p.value.toLowerCase().includes(q)
    )
  }
  return points
})

function handleRowClick(index: number, event: MouseEvent) {
  if (event.ctrlKey || event.metaKey) {
    if (selectedIndices.value.has(index)) {
      selectedIndices.value.delete(index)
    } else {
      selectedIndices.value.add(index)
    }
  } else if (event.shiftKey && lastClickedIndex.value >= 0) {
    const start = Math.min(lastClickedIndex.value, index)
    const end = Math.max(lastClickedIndex.value, index)
    for (let i = start; i <= end; i++) {
      selectedIndices.value.add(i)
    }
  } else {
    selectedIndices.value.clear()
    selectedIndices.value.add(index)
  }
  lastClickedIndex.value = index
  emitSelection()
}

function emitSelection() {
  const selected: ReceivedDataPointInfo[] = []
  for (const idx of selectedIndices.value) {
    if (filteredPoints.value[idx]) {
      selected.push(filteredPoints.value[idx])
    }
  }
  emit('point-select', selected)
}

function qualityLabel(point: ReceivedDataPointInfo): string {
  return point.quality_iv ? 'IV' : 'OK'
}

function qualityClass(point: ReceivedDataPointInfo): string {
  return point.quality_iv ? 'quality-iv' : 'quality-ok'
}

const categoryTitle = computed(() => {
  if (!selectedCategory.value) return '全部数据'
  return selectedCategory.value
})

const pointCount = computed(() => filteredPoints.value.length)
</script>

<template>
  <div class="data-table-container">
    <div v-if="!selectedConnectionId" class="empty-state">
      选择一个连接查看数据
    </div>

    <template v-else>
      <div class="table-header">
        <span class="header-title">{{ categoryTitle }}</span>
        <input
          v-model="searchFilter"
          class="search-input"
          type="text"
          placeholder="搜索 IOA / 类型..."
        />
        <span class="point-count">{{ pointCount }} 个</span>
      </div>

      <div v-if="allPoints.length === 0" class="empty-state">
        <div>
          <div>暂无数据</div>
          <div class="empty-hint">请先发送总召唤获取数据</div>
        </div>
      </div>

      <div v-else class="table-scroll">
        <table class="table">
          <thead>
            <tr>
              <th class="col-ioa">IOA</th>
              <th class="col-type">类型</th>
              <th class="col-value">值</th>
              <th class="col-quality">品质</th>
              <th class="col-timestamp">时间戳</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(point, index) in filteredPoints"
              :key="point.ioa"
              :class="{
                selected: selectedIndices.has(index),
                'value-changed': changedIoas.has(point.ioa),
              }"
              @click="handleRowClick(index, $event)"
            >
              <td class="col-ioa">{{ point.ioa }}</td>
              <td class="col-type">{{ point.asdu_type }}</td>
              <td class="col-value">{{ point.value }}</td>
              <td :class="['col-quality', qualityClass(point)]">{{ qualityLabel(point) }}</td>
              <td class="col-timestamp">{{ point.timestamp ?? '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </div>
</template>

<style scoped>
.data-table-container {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #6c7086;
  font-size: 13px;
  text-align: center;
}

.empty-hint {
  font-size: 11px;
  color: #45475a;
  margin-top: 6px;
}

.table-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid #313244;
  flex-shrink: 0;
  background: #1e1e2e;
}

.header-title {
  font-size: 12px;
  font-weight: 600;
  color: #89b4fa;
  white-space: nowrap;
}

.search-input {
  flex: 1;
  max-width: 200px;
  padding: 3px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
  margin-left: auto;
}

.search-input:focus {
  outline: none;
  border-color: #89b4fa;
}

.point-count {
  font-size: 11px;
  color: #6c7086;
  white-space: nowrap;
}

.table-scroll {
  flex: 1;
  overflow-y: auto;
}

.table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.table th {
  position: sticky;
  top: 0;
  background: #1e1e2e;
  color: #6c7086;
  font-weight: 500;
  padding: 6px 10px;
  text-align: left;
  border-bottom: 1px solid #313244;
  z-index: 1;
}

.table tbody tr {
  cursor: pointer;
}

.table tbody tr:hover {
  background: #1e1e2e;
}

.table tbody tr.selected {
  background: #89b4fa;
  color: #1e1e2e;
}

.table tbody tr.selected .col-ioa,
.table tbody tr.selected .col-quality,
.table tbody tr.selected .col-timestamp {
  color: #1e1e2e;
}

.table tbody tr.value-changed {
  animation: flash-highlight 2s ease-out;
}

@keyframes flash-highlight {
  0% { background: rgba(250, 179, 135, 0.3); }
  100% { background: transparent; }
}

.table td {
  padding: 4px 10px;
  border-bottom: 1px solid #1e1e2e;
}

.col-ioa {
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 80px;
  color: #89b4fa;
}

.col-type {
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 120px;
}

.col-value {
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.col-quality {
  width: 50px;
  font-weight: 600;
  font-size: 11px;
}

.col-quality.quality-ok {
  color: #a6e3a1;
}

.col-quality.quality-iv {
  color: #f38ba8;
}

.col-timestamp {
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 120px;
  color: #6c7086;
}
</style>
