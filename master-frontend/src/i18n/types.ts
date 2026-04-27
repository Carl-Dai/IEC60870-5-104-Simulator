export type Locale = 'zh-CN' | 'en-US'
export const SUPPORTED_LOCALES: readonly Locale[] = ['zh-CN', 'en-US'] as const
export const STORAGE_KEY = 'iec104.locale'
