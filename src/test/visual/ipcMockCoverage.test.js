// Guards the visual lane against IPC mock drift.
//
// `ipcVisualMocks.js` replaces the whole `src/lib/ipc.js` module with a factory,
// so any export the map lacks is simply absent at import time: a component that
// imports it fails the visual spec with an opaque "does not provide an export"
// error instead of a missing-mock message. This test names the gap directly.
import { describe, expect, it, vi } from 'vitest'

import { visualIpcMocks } from './ipcVisualMocks.js'

const actualIpc = await vi.importActual('../../lib/ipc.js')

describe('visual IPC mock registry', () => {
  it('mocks every export src/lib/ipc.js provides', () => {
    const missing = Object.keys(actualIpc)
      .filter((name) => !Object.hasOwn(visualIpcMocks, name))
      .sort()

    expect(missing).toEqual([])
  })

  it('mocks nothing src/lib/ipc.js does not export', () => {
    const stale = Object.keys(visualIpcMocks)
      .filter((name) => !Object.hasOwn(actualIpc, name))
      .sort()

    expect(stale).toEqual([])
  })
})
