import { vi } from 'vitest'

// Node 25 ships a built-in localStorage that lacks clear/setItem/getItem.
// Replace it with a proper in-memory implementation before every test file.
function createLocalStorageMock() {
  let store: Record<string, string> = {}
  return {
    getItem: (key: string) => (key in store ? store[key] : null),
    setItem: (key: string, value: string) => { store[key] = String(value) },
    removeItem: (key: string) => { delete store[key] },
    clear: () => { store = {} },
    get length() { return Object.keys(store).length },
    key: (i: number) => Object.keys(store)[i] ?? null,
  }
}

vi.stubGlobal('localStorage', createLocalStorageMock())
