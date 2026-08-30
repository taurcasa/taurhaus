// @vitest-environment node
import { describe, it, expect } from 'vitest'

import { resolveVisualBrowser, SYSTEM_CHROME } from './visual-browser.mjs'

const MANAGED_ROOT = '/home/tester/.cache/ms-playwright'

// A fake filesystem: a set of paths that exist, plus the directory listings the
// managed-Chromium scan reads.
function fakeHost({ paths = [], entries = {}, env = {}, playwrightPath = null } = {}) {
  const present = new Set(paths)
  return {
    env: { HOME: '/home/tester', ...env },
    platform: 'linux',
    exists: (candidate) => present.has(candidate),
    readDir: (dir) => entries[dir] ?? [],
    playwrightExecutablePath: () => {
      if (!playwrightPath) throw new Error('playwright is not installed')
      return playwrightPath
    },
  }
}

describe('visual lane browser resolution', () => {
  it('uses PLAYWRIGHT_CHROME_PATH when it points at an existing binary', () => {
    const resolved = resolveVisualBrowser(fakeHost({
      env: { PLAYWRIGHT_CHROME_PATH: '/opt/chrome/chrome' },
      paths: ['/opt/chrome/chrome', SYSTEM_CHROME],
    }))

    expect(resolved.executablePath).toBe('/opt/chrome/chrome')
    expect(resolved.source).toBe('PLAYWRIGHT_CHROME_PATH')
  })

  it('fails fast with the offending path when PLAYWRIGHT_CHROME_PATH does not exist', () => {
    const host = fakeHost({
      env: { PLAYWRIGHT_CHROME_PATH: '/nonexistent' },
      paths: [SYSTEM_CHROME],
    })

    expect(() => resolveVisualBrowser(host)).toThrow(/PLAYWRIGHT_CHROME_PATH/)
    expect(() => resolveVisualBrowser(host)).toThrow(/\/nonexistent/)
  })

  it('falls back to the system google-chrome when no override is set', () => {
    const resolved = resolveVisualBrowser(fakeHost({
      paths: [SYSTEM_CHROME],
      playwrightPath: `${MANAGED_ROOT}/chromium-1234/chrome-linux64/chrome`,
    }))

    expect(resolved.executablePath).toBe(SYSTEM_CHROME)
    expect(resolved.source).toBe('system google-chrome')
  })

  it("uses the playwright package's own Chromium when that binary is installed", () => {
    const managed = `${MANAGED_ROOT}/chromium-1234/chrome-linux64/chrome`
    const resolved = resolveVisualBrowser(fakeHost({
      paths: [managed],
      playwrightPath: managed,
    }))

    expect(resolved.executablePath).toBe(managed)
    expect(resolved.source).toBe('playwright-managed chromium')
  })

  it('scans the playwright browser cache when the pinned revision is not installed', () => {
    const installed = `${MANAGED_ROOT}/chromium-1234/chrome-linux64/chrome`
    const older = `${MANAGED_ROOT}/chromium-1187/chrome-linux64/chrome`
    const resolved = resolveVisualBrowser(fakeHost({
      // The playwright package pins a revision this host never downloaded.
      playwrightPath: `${MANAGED_ROOT}/chromium-1208/chrome-linux64/chrome`,
      paths: [installed, older],
      entries: {
        [MANAGED_ROOT]: ['chromium-1187', 'chromium-1234', 'chromium_headless_shell-1234', 'ffmpeg-1011'],
      },
    }))

    // Highest installed revision wins, and the headless shell is never picked.
    expect(resolved.executablePath).toBe(installed)
    expect(resolved.source).toBe('playwright-managed chromium')
  })

  it('honours PLAYWRIGHT_BROWSERS_PATH for the cache scan', () => {
    const installed = '/srv/browsers/chromium-1234/chrome-linux64/chrome'
    const resolved = resolveVisualBrowser(fakeHost({
      env: { PLAYWRIGHT_BROWSERS_PATH: '/srv/browsers' },
      paths: [installed],
      entries: { '/srv/browsers': ['chromium-1234'] },
    }))

    expect(resolved.executablePath).toBe(installed)
  })

  it('explains where it looked when no browser is available at all', () => {
    expect(() => resolveVisualBrowser(fakeHost({}))).toThrow(/playwright install chromium/)
  })
})
