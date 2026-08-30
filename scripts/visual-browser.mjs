// Browser resolution for the browser-mode visual lane (`just test-visual`).
//
// The lane used to hardcode `/usr/bin/google-chrome`, so it could not run on a
// host that only has Playwright's managed Chromium. Resolution order:
//
//   1. PLAYWRIGHT_CHROME_PATH — an explicit override; a path that does not
//      exist is an error naming the path, never a silent fallback.
//   2. /usr/bin/google-chrome, when it exists.
//   3. Playwright's managed Chromium: the revision the installed `playwright`
//      package points at, else the newest `chromium-<revision>` actually
//      present in the browser cache.
//
// The resolved path is always passed to Playwright as `executablePath`, so the
// binary reported at startup is exactly the one that launches.
import { existsSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

export const SYSTEM_CHROME = '/usr/bin/google-chrome'

// Per-platform layout of a `chromium-<revision>` directory in the cache.
const MANAGED_BINARIES = {
  linux: ['chrome-linux64/chrome', 'chrome-linux/chrome'],
  darwin: ['chrome-mac/Chromium.app/Contents/MacOS/Chromium'],
  win32: ['chrome-win/chrome.exe'],
}

function defaultCacheRoot(env, platform) {
  const configured = env.PLAYWRIGHT_BROWSERS_PATH
  if (configured && configured !== '0') return configured
  const home = env.HOME || env.USERPROFILE || ''
  if (platform === 'darwin') return join(home, 'Library', 'Caches', 'ms-playwright')
  if (platform === 'win32') return join(home, 'AppData', 'Local', 'ms-playwright')
  return join(home, '.cache', 'ms-playwright')
}

function newestManagedChromium({ cacheRoot, platform, exists, readDir }) {
  const revisions = readDir(cacheRoot)
    .map((entry) => /^chromium-(\d+)$/.exec(entry))
    .filter(Boolean)
    .map((match) => ({ dir: match[0], revision: Number(match[1]) }))
    .sort((a, b) => b.revision - a.revision)

  for (const { dir } of revisions) {
    for (const relative of MANAGED_BINARIES[platform] ?? MANAGED_BINARIES.linux) {
      const candidate = join(cacheRoot, dir, relative)
      if (exists(candidate)) return candidate
    }
  }
  return null
}

/**
 * Resolve the browser the visual lane should launch.
 *
 * @returns {{ executablePath: string, source: string }}
 */
export function resolveVisualBrowser({
  env = process.env,
  platform = process.platform,
  exists = existsSync,
  readDir = (dir) => (existsSync(dir) ? readdirSync(dir) : []),
  playwrightExecutablePath,
} = {}) {
  const override = env.PLAYWRIGHT_CHROME_PATH
  if (override) {
    if (!exists(override)) {
      throw new Error(
        `PLAYWRIGHT_CHROME_PATH points at a browser that does not exist: ${override}`,
      )
    }
    return { executablePath: override, source: 'PLAYWRIGHT_CHROME_PATH' }
  }

  if (exists(SYSTEM_CHROME)) {
    return { executablePath: SYSTEM_CHROME, source: 'system google-chrome' }
  }

  let pinned = null
  try {
    pinned = playwrightExecutablePath?.() ?? null
  } catch {
    pinned = null
  }
  if (pinned && exists(pinned)) {
    return { executablePath: pinned, source: 'playwright-managed chromium' }
  }

  const cacheRoot = defaultCacheRoot(env, platform)
  const scanned = newestManagedChromium({ cacheRoot, platform, exists, readDir })
  if (scanned) {
    return { executablePath: scanned, source: 'playwright-managed chromium' }
  }

  throw new Error(
    'No browser found for the visual lane. Looked at PLAYWRIGHT_CHROME_PATH (unset), ' +
      `${SYSTEM_CHROME}${pinned ? `, ${pinned}` : ''}, and ${cacheRoot}. ` +
      'Install one with `bunx playwright install chromium`, or set PLAYWRIGHT_CHROME_PATH.',
  )
}
