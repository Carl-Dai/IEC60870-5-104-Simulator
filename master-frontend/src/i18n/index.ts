import { ref, computed } from 'vue'
import type { Locale } from './types'
import { SUPPORTED_LOCALES, STORAGE_KEY } from './types'
import { detectSystemLocale } from './detect'
import zhCN from './locales/zh-CN'
import enUS from './locales/en-US'
import type { DictShape } from './locales/zh-CN'

const dictionaries: Record<Locale, DictShape> = {
  'zh-CN': zhCN,
  'en-US': enUS,
}

function initialLocale(): Locale {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved && (SUPPORTED_LOCALES as readonly string[]).includes(saved)) {
      return saved as Locale
    }
  } catch { /* ignore */ }
  return detectSystemLocale()
}

export const locale = ref<Locale>(initialLocale())

function lookup(dict: DictShape, key: string): string | undefined {
  const parts = key.split('.')
  let cur: unknown = dict
  for (const p of parts) {
    if (cur && typeof cur === 'object' && p in (cur as Record<string, unknown>)) {
      cur = (cur as Record<string, unknown>)[p]
    } else {
      return undefined
    }
  }
  return typeof cur === 'string' ? cur : undefined
}

function interpolate(template: string, params?: Record<string, unknown>): string {
  if (!params) return template
  return template.replace(/\{(\w+)\}/g, (_, name) => {
    const v = params[name]
    return v === undefined || v === null ? `{${name}}` : String(v)
  })
}

function translate(key: string, params?: Record<string, unknown>): string {
  const fromCurrent = lookup(dictionaries[locale.value], key)
  if (fromCurrent !== undefined) return interpolate(fromCurrent, params)
  const fromFallback = lookup(dictionaries['en-US'], key)
  if (fromFallback !== undefined) return interpolate(fromFallback, params)
  return key
}

function setLocale(next: Locale) {
  if (!(SUPPORTED_LOCALES as readonly string[]).includes(next)) return
  locale.value = next
  try { localStorage.setItem(STORAGE_KEY, next) } catch { /* ignore */ }
}

const localeRef = computed(() => locale.value)

export function useI18n() {
  return { t: translate, locale: localeRef, setLocale }
}

// Test-only: re-runs initial locale detection. Don't use in production code.
export function __resetForTests() {
  locale.value = initialLocale()
}

export type { Locale }
