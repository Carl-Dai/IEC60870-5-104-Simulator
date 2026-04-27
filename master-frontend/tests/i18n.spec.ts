import { describe, it, expect, vi, beforeEach } from 'vitest'
import { detectSystemLocale } from '../src/i18n/detect'
import { useI18n, __resetForTests, locale } from '../src/i18n'
import { nextTick } from 'vue'

describe('detectSystemLocale', () => {
  it('returns zh-CN when navigator.language starts with zh', () => {
    vi.stubGlobal('navigator', { language: 'zh-CN' })
    expect(detectSystemLocale()).toBe('zh-CN')
  })
  it('returns zh-CN for zh-TW etc.', () => {
    vi.stubGlobal('navigator', { language: 'zh-TW' })
    expect(detectSystemLocale()).toBe('zh-CN')
  })
  it('returns en-US for English', () => {
    vi.stubGlobal('navigator', { language: 'en-US' })
    expect(detectSystemLocale()).toBe('en-US')
  })
  it('returns en-US as fallback for other locales', () => {
    vi.stubGlobal('navigator', { language: 'ja-JP' })
    expect(detectSystemLocale()).toBe('en-US')
  })
})

describe('useI18n', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.stubGlobal('navigator', { language: 'zh-CN' })
  })

  it('t() returns current locale string', () => {
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('toolbar.newConnection')).toBe('新建连接')
    setLocale('en-US')
    expect(t('toolbar.newConnection')).toBe('New Connection')
  })

  it('t() interpolates {placeholders}', () => {
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('_test.interp', { id: 42, user: 'alice' })).toBe('订单 #42 由 alice 创建')
    setLocale('en-US')
    expect(t('_test.interp', { id: 42, user: 'alice' })).toBe('Order #42 created by alice')
  })

  it('t() leaves unknown placeholders intact', () => {
    const { t, setLocale } = useI18n()
    setLocale('en-US')
    expect(t('_test.interp', { id: 1 })).toBe('Order #1 created by {user}')
  })

  it('returns key when missing in both locales', () => {
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    // @ts-expect-error intentional missing key
    expect(t('does.not.exist')).toBe('does.not.exist')
  })

  it('setLocale writes to localStorage', () => {
    const { setLocale } = useI18n()
    setLocale('en-US')
    expect(localStorage.getItem('iec104.locale')).toBe('en-US')
  })

  it('locale is reactive', async () => {
    const { t, locale, setLocale } = useI18n()
    setLocale('zh-CN')
    setLocale('en-US')
    await nextTick()
    expect(t('common.cancel')).toBe('Cancel')
    expect(locale.value).toBe('en-US')
  })

  it('setLocale ignores invalid values', () => {
    const { setLocale, locale } = useI18n()
    setLocale('zh-CN')
    setLocale('fr-FR' as any)
    expect(locale.value).toBe('zh-CN')
  })
})

describe('initial locale boot', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('boots zh-CN when navigator is Chinese and no saved locale', () => {
    vi.stubGlobal('navigator', { language: 'zh-CN' })
    __resetForTests()
    expect(locale.value).toBe('zh-CN')
  })

  it('boots en-US when navigator is English and no saved locale', () => {
    vi.stubGlobal('navigator', { language: 'en-US' })
    __resetForTests()
    expect(locale.value).toBe('en-US')
  })

  it('boots from saved localStorage value, ignoring navigator', () => {
    vi.stubGlobal('navigator', { language: 'en-US' })
    localStorage.setItem('iec104.locale', 'zh-CN')
    __resetForTests()
    expect(locale.value).toBe('zh-CN')
  })

  it('ignores invalid saved locale and falls back to navigator', () => {
    vi.stubGlobal('navigator', { language: 'zh-CN' })
    localStorage.setItem('iec104.locale', 'fr-FR')
    __resetForTests()
    expect(locale.value).toBe('zh-CN')
  })
})
