import type { Locale } from './types'

export function detectSystemLocale(): Locale {
  const lang = (typeof navigator !== 'undefined' && navigator.language) || ''
  return lang.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}
