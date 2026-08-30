import { mkdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, isAbsolute, resolve } from 'node:path'

import { defineBrowserCommand, playwright } from '@vitest/browser-playwright'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { chromium } from 'playwright'
import { defineConfig } from 'vitest/config'

import { resolveVisualBrowser } from './scripts/visual-browser.mjs'

const browser = resolveVisualBrowser({
  playwrightExecutablePath: () => chromium.executablePath(),
})
// One startup line, naming the exact binary the lane launches. Vitest loads
// this config twice per run, so the banner is deduplicated per process.
const bannerKey = Symbol.for('taurhaus.visual.browser.banner')
if (globalThis[bannerKey] !== browser.executablePath) {
  globalThis[bannerKey] = browser.executablePath
  console.log(`[visual] browser: ${browser.executablePath} (${browser.source})`)
}
const screenshotRoot = resolve(process.cwd(), 'src/test/visual/__screenshots__')

function resolveArtifactPath(candidate) {
  if (!candidate) {
    throw new Error('Screenshot path is required.')
  }
  return isAbsolute(candidate) ? candidate : resolve(screenshotRoot, candidate)
}

function prepareArtifactPath(candidate) {
  const artifactPath = resolveArtifactPath(candidate)
  mkdirSync(dirname(artifactPath), { recursive: true })
  return artifactPath
}

function readPngMeta(filePath) {
  const buffer = readFileSync(filePath)
  const signature = buffer.subarray(0, 8)
  const expectedSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])
  const validSignature = signature.equals(expectedSignature)

  return {
    path: filePath,
    size: statSync(filePath).size,
    isPng: validSignature,
    width: validSignature ? buffer.readUInt32BE(16) : 0,
    height: validSignature ? buffer.readUInt32BE(20) : 0,
  }
}

export default defineConfig({
  plugins: [svelte({ hot: false }), tailwindcss()],
  resolve: {
    conditions: ['browser'],
  },
  test: {
    include: ['src/test/visual/specs/**/*.visual.test.js'],
    globals: true,
    setupFiles: [],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright({
        launchOptions: {
          executablePath: browser.executablePath,
          args: ['--no-sandbox', '--disable-dev-shm-usage'],
        },
      }),
      instances: [
        {
          browser: 'chromium',
        },
      ],
      viewport: {
        width: 960,
        height: 640,
      },
      screenshotDirectory: 'src/test/visual/__screenshots__',
      screenshotFailures: true,
      fileParallelism: false,
      commands: {
        resolveVisualArtifactPath: defineBrowserCommand(async (_, screenshotPath) => {
          return prepareArtifactPath(screenshotPath)
        }),
        readVisualArtifact: defineBrowserCommand(async (_, screenshotPath) => {
          const resolvedPath = resolveArtifactPath(screenshotPath)
          return readPngMeta(resolvedPath)
        }),
      },
    },
  },
})
