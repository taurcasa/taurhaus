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

  // Regression: the cache scan searched `chrome-mac/Chromium.app` and
  // `chrome-win/chrome.exe`, the pre-1.58 layouts; Playwright 1.58 installs
  // Chrome for Testing builds under `chrome-mac-{x64,arm64}` and
  // `chrome-win64`, so on those platforms the fallback could never find a
  // revision the package did not pin. (Codex review of the visual lane fix.)
  it('scans the Chrome for Testing layout on macOS, preferring the host architecture', () => {
    const cache = '/Users/tester/Library/Caches/ms-playwright'
    const arm = `${cache}/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`
    const x64 = `${cache}/chromium-1234/chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`
    const host = fakeHost({
      env: { HOME: '/Users/tester' },
      paths: [arm, x64],
      entries: { [cache]: ['chromium-1234'] },
    })

    expect(resolveVisualBrowser({ ...host, platform: 'darwin', arch: 'arm64' }).executablePath).toBe(arm)
    expect(resolveVisualBrowser({ ...host, platform: 'darwin', arch: 'x64' }).executablePath).toBe(x64)
  })

  it('takes the other architecture or the legacy Chromium.app on macOS when that is all the cache holds', () => {
    const cache = '/Users/tester/Library/Caches/ms-playwright'
    const x64 = `${cache}/chromium-1234/chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`
    const legacy = `${cache}/chromium-1100/chrome-mac/Chromium.app/Contents/MacOS/Chromium`
    const onlyX64 = fakeHost({ env: { HOME: '/Users/tester' }, paths: [x64], entries: { [cache]: ['chromium-1234'] } })
    expect(resolveVisualBrowser({ ...onlyX64, platform: 'darwin', arch: 'arm64' }).executablePath).toBe(x64)

    const onlyLegacy = fakeHost({ env: { HOME: '/Users/tester' }, paths: [legacy], entries: { [cache]: ['chromium-1100'] } })
    expect(resolveVisualBrowser({ ...onlyLegacy, platform: 'darwin', arch: 'arm64' }).executablePath).toBe(legacy)
  })

  it('scans the chrome-win64 layout on Windows, then the legacy chrome-win one', () => {
    const cache = 'C:\\Users\\tester\\AppData\\Local\\ms-playwright'
    const current = `${cache}/chromium-1234/chrome-win64/chrome.exe`
    const legacy = `${cache}/chromium-1100/chrome-win/chrome.exe`
    const host = fakeHost({
      env: { PLAYWRIGHT_BROWSERS_PATH: cache },
      paths: [current, legacy],
      entries: { [cache]: ['chromium-1100', 'chromium-1234'] },
    })
    expect(resolveVisualBrowser({ ...host, platform: 'win32', arch: 'x64' }).executablePath).toBe(current)

    const legacyOnly = fakeHost({ env: { PLAYWRIGHT_BROWSERS_PATH: cache }, paths: [legacy], entries: { [cache]: ['chromium-1100'] } })
    expect(resolveVisualBrowser({ ...legacyOnly, platform: 'win32', arch: 'x64' }).executablePath).toBe(legacy)
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
