<script setup lang="ts">
import { useI18n } from '../i18n'

const { locale, setLocale } = useI18n()

function toggle() {
  setLocale(locale.value === 'zh-CN' ? 'en-US' : 'zh-CN')
}
</script>

<template>
  <button class="lang-switch" :title="locale === 'zh-CN' ? 'Switch to English' : '切换到中文'" @click="toggle">
    <span :class="['seg', { active: locale === 'zh-CN' }]">中</span>
    <span class="sep">/</span>
    <span :class="['seg', { active: locale === 'en-US' }]">EN</span>
  </button>
</template>

<style scoped>
.lang-switch {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  margin-right: 8px;
  background: transparent;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #6c7086;
  cursor: pointer;
  font-size: 12px;
  font-family: inherit;
}
.lang-switch:hover { color: #cdd6f4; background: #313244; }
.seg { padding: 0 2px; }
.seg.active { color: #cdd6f4; font-weight: 600; }
.sep { color: #45475a; }
</style>
