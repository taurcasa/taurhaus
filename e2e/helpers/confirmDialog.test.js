import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { isConfirmDialogOpen, clickOpenConfirmDialog } from './confirmDialog.js'

// Regression: 275d42d6 duplicated the open predicate and bypassed fastClick's
// intercepted-click fallback in runtime disband cleanup.
describe('open confirmation dialog', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <dialog data-testid="confirm-dialog"><button data-testid="confirm-dialog-confirm">Hidden</button></dialog>
      <dialog open data-testid="confirm-dialog"><button data-testid="confirm-dialog-confirm">Confirm</button></dialog>`
    vi.stubGlobal('browser', { execute: async (fn, arg) => fn(arg) })
    vi.stubGlobal('$', async (selector) => ({
      isExisting: async () => document.querySelector(selector) !== null,
      isEnabled: async () => !document.querySelector(selector).disabled,
      scrollIntoView: async () => {},
      click: async () => { throw new Error('element click intercepted') },
    }))
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    document.body.innerHTML = ''
  })

  it('requires an open dialog, not just a mounted confirmation', async () => {
    expect(await isConfirmDialogOpen()).toBe(true)
    document.querySelector('dialog[open]').removeAttribute('open')
    expect(await isConfirmDialogOpen()).toBe(false)
  })

  it('recovers an intercepted confirmation click and never clicks a closed dialog', async () => {
    const hidden = vi.fn()
    const confirm = vi.fn()
    document.querySelector('dialog:not([open]) button').onclick = hidden
    document.querySelector('dialog[open] button').onclick = confirm
    await clickOpenConfirmDialog()
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(hidden).not.toHaveBeenCalled()
  })

  it('fails when the open confirmation is disabled or missing', async () => {
    document.querySelector('dialog[open] button').disabled = true
    await expect(clickOpenConfirmDialog()).rejects.toThrow('Open confirmation action was unavailable')
    document.querySelector('dialog[open]').remove()
    await expect(clickOpenConfirmDialog()).rejects.toThrow('Open confirmation action was unavailable')
  })
})
