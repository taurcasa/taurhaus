import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clickActiveSlideOverTestId } from './slideover.js'

vi.mock('../helpers.js', () => ({ waitForAppReady: vi.fn(), ensureMainApp: async () => true }))
vi.mock('./navigation.js', () => ({ waitForProjectsLoaded: vi.fn(), clickTestId: vi.fn(), fastClick: vi.fn() }))
vi.mock('./tmux.js', () => ({ snapshotTmuxPanes: vi.fn(), cleanupNewTmuxPanes: vi.fn() }))
vi.mock('./laneTmux.js', () => ({ assertTmuxIsolation: vi.fn() }))
vi.mock('./slideover.js', () => ({
  isSlideOverOpen: async () => false,
  hasActiveSlideOverTestId: async () => true,
  clickActiveSlideOverTestId: vi.fn(),
  setActiveSlideOverInputValue: async () => true,
  readActiveSlideOverInputValue: async () => 'Role instructions v2',
}))

// Regression: 707ce88a opened role inspection once immediately after save;
// persistence can finish before the editor yields to the refreshed catalog.
// Execute the real edit case with fake IPC/DOM and a delayed inspect control.
describe('template edit inspection', () => {
  let setup
  let edit
  let detailOpen
  let attempts

  beforeEach(async () => {
    vi.useFakeTimers()
    vi.resetModules()
    detailOpen = false
    attempts = 0
    vi.stubGlobal('describe', (_name, register) => register())
    vi.stubGlobal('before', (hook) => { setup = hook })
    vi.stubGlobal('after', vi.fn())
    vi.stubGlobal('it', (name, run) => { if (name === 'edits a custom role via UI') edit = run })
    vi.stubGlobal('$', async (selector) => ({
      isExisting: async () => selector.includes('template-role-detail') ? detailOpen
        : !/mesh-mode-gate|mesh-mode-runtime|mesh-mode-setup|mesh-availability-blocking/.test(selector),
    }))
    vi.stubGlobal('browser', {
      executeAsync: async () => ({ ok: true, result: { instructions: 'Role instructions v2' } }),
      waitUntil: async (condition, options) => {
        for (let poll = 0; poll < 3; poll++) if (await condition()) return true
        throw new Error(options.timeoutMsg)
      },
    })
    clickActiveSlideOverTestId.mockReset().mockImplementation(async (id) => {
      if (/^role-inspect-|^role-template-card-/.test(id)) {
        if (++attempts < 3) return false
        detailOpen = true
      }
      return true
    })
    await import('../specs/template-crud-ui.js')
    await setup()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.clearAllTimers()
    vi.useRealTimers()
  })

  it('waits for the inspect control without repeating the save', async () => {
    const context = { skip: vi.fn() }
    await edit.call(context)
    expect(detailOpen).toBe(true)
    expect(clickActiveSlideOverTestId.mock.calls.filter(([id]) => id === 'role-editor-save')).toHaveLength(1)
    expect(context.skip).not.toHaveBeenCalled()
  })

  it('fails when no inspect control ever appears', async () => {
    clickActiveSlideOverTestId.mockImplementation(async (id) => !/^role-inspect-|^role-template-card-/.test(id))
    await expect(edit.call({ skip: vi.fn() })).rejects.toThrow('Role detail panel did not open after edit')
  })
})
