import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ensureMainApp } from '../helpers.js'

vi.mock('../helpers.js', () => ({
  waitForAppReady: vi.fn(),
  ensureMainApp: vi.fn(async () => true),
}))
vi.mock('./navigation.js', () => ({
  waitForProjectsLoaded: vi.fn(), clickTestId: vi.fn(), switchToTab: vi.fn(),
}))
vi.mock('./tmux.js', () => ({ snapshotTmuxPanes: vi.fn(), cleanupNewTmuxPanes: vi.fn() }))
vi.mock('./laneTmux.js', () => ({ assertTmuxIsolation: vi.fn() }))
vi.mock('./meshBuilder.js', () => ({ setInlineBuilderTeamName: vi.fn() }))

// Regression: 5cebfef81 made a one-shot prerequisite probe decide whether
// tier-2 ran; acd3c5aa3 also converted transient setup failures into skips.
// Execute the real spec hooks/cases with fake IPC/DOM, never a CLI or daemon.
describe('Mesh workflow coverage preconditions', () => {
  let setup
  let cases
  const available = { canInitialize: true, meshAvailable: true, tmuxAvailable: true, blockingErrors: [] }

  beforeEach(async () => {
    vi.resetModules()
    cases = new Map()
    vi.stubGlobal('describe', (_name, register) => register())
    vi.stubGlobal('before', (hook) => { setup = hook })
    vi.stubGlobal('after', vi.fn())
    vi.stubGlobal('it', (name, run) => cases.set(name, run))
    vi.stubGlobal('$', vi.fn(async (selector) => ({
      isExisting: async () => !selector.includes('mesh-mode-gate') && !selector.includes('mesh-mode-runtime'),
    })))
    vi.stubGlobal('browser', {
      executeAsync: vi.fn(async () => ({ ok: true, result: available })),
      waitUntil: vi.fn(async (condition, options) => {
        if (await condition()) return true
        throw new Error(options.timeoutMsg)
      }),
    })
    await import('../specs/mesh-workflow.js')
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('fails setup when the prerequisite IPC fails, instead of disabling tier 2', async () => {
    browser.executeAsync.mockResolvedValue({ ok: false, error: 'fixture RPC timeout' })
    await expect(setup()).rejects.toThrow('fixture RPC timeout')
  })

  it('fails when the main app cannot be established', async () => {
    ensureMainApp.mockResolvedValueOnce(false)
    await expect(setup()).rejects.toThrow('Main app unavailable')
  })

  it.each([
    { ...available, tmuxAvailable: false, canInitialize: false, blockingErrors: ['fixture tmux unavailable'] },
    { ...available, canInitialize: false, blockingErrors: ['fixture initialization unavailable'] },
    {},
  ])('fails incomplete or blocked prerequisite reports: %j', async (report) => {
    browser.executeAsync.mockResolvedValue({ ok: true, result: report })
    await expect(setup()).rejects.toThrow(/prerequisite/i)
  })

  it.each([
    'shows setup controls in setup mode',
    'initializes an e2e team, hot-adds an agent, then disbands',
  ])('fails a transient blocking surface without skipping: %s', async (name) => {
    await setup()
    const context = { skip: vi.fn() }
    await expect(cases.get(name).call(context)).rejects.toThrow(/setup/i)
    expect(context.skip).not.toHaveBeenCalled()
  })

  it('runs setup assertions when all prerequisites and the setup surface are ready', async () => {
    await setup()
    $.mockImplementation(async (selector) => ({
      isExisting: async () => !/mesh-mode-gate|mesh-mode-runtime|mesh-availability-blocking/.test(selector),
    }))
    const context = { skip: vi.fn() }
    await cases.get('shows setup controls in setup mode').call(context)
    expect(context.skip).not.toHaveBeenCalled()
  })

  it.each(['mesh-init-failure', 'mesh-error'])('fails initialization errors without skipping: %s', async (failure) => {
    await setup()
    $.mockImplementation(async (selector) => ({
      isExisting: async () => {
        if (selector.includes(failure)) return true
        return !/mesh-mode-gate|mesh-mode-runtime|mesh-availability-blocking|mesh-init-failure|mesh-error/.test(selector)
      },
      waitForExist: async () => {},
      isEnabled: async () => true,
      click: async () => {},
      getText: async () => 'fixture initialization failure',
    }))
    const context = { skip: vi.fn() }
    await expect(cases.get('initializes an e2e team, hot-adds an agent, then disbands').call(context))
      .rejects.toThrow('fixture initialization failure')
    expect(context.skip).not.toHaveBeenCalled()
  })
})
