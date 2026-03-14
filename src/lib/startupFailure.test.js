import { describe, expect, it, beforeEach } from 'vitest'
import { extractStartupErrorMessage, renderStartupFailure } from './startupFailure.js'

describe('startupFailure', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>'
  })

  it('extracts a message from Error objects', () => {
    expect(extractStartupErrorMessage(new Error('boom'))).toBe('boom')
  })

  it('falls back for empty input', () => {
    expect(extractStartupErrorMessage(null)).toBe('Taurhaus failed to start. Check logs for details.')
  })

  it('renders a visible startup failure overlay', () => {
    renderStartupFailure(new Error('white screen regression'))

    const overlay = document.getElementById('startup-failure-overlay')
    expect(overlay).toBeTruthy()
    expect(overlay.textContent).toContain('Startup failed')
    expect(overlay.textContent).toContain('white screen regression')
  })
})
