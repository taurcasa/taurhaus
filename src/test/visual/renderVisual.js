import '../../app.css'

import { cleanup, render } from '@testing-library/svelte'
import { commands, page } from 'vitest/browser'

import { resetVisualIpcMocks } from './ipcVisualMocks.js'

async function waitForAnimationFrame() {
  await new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

export async function waitForVisualReady() {
  if (document.fonts?.ready) {
    await document.fonts.ready
  }
  await waitForAnimationFrame()
  await waitForAnimationFrame()
}

export function applyVisualTheme(theme) {
  const dark = theme === 'dark'
  document.documentElement.classList.toggle('dark', dark)
  document.documentElement.dataset.theme = theme
  document.body.dataset.theme = theme
  document.body.style.margin = '0'
  document.body.style.minHeight = '100vh'
  document.body.style.background = dark ? '#0A2E2B' : '#F0FDFA'
  document.body.style.color = dark ? '#f4f4f5' : '#18181b'
  return dark
}

export async function renderVisual(Component, {
  props = {},
  theme = 'light',
  viewport = { width: 960, height: 640 },
  ipc = {},
} = {}) {
  cleanup()
  resetVisualIpcMocks(ipc)
  await page.viewport(viewport.width, viewport.height)
  const dark = applyVisualTheme(theme)
  const result = render(Component, {
    props: {
      dark,
      ...props,
    },
  })
  await waitForVisualReady()
  return {
    ...result,
    dark,
    theme,
    viewport,
  }
}

export async function captureVisual(path, options = {}) {
  await waitForVisualReady()
  const artifactPath = await commands.resolveVisualArtifactPath(path)
  return page.screenshot({
    path: artifactPath,
    ...options,
  })
}

export async function captureVisualBase64(path, options = {}) {
  await waitForVisualReady()
  const artifactPath = await commands.resolveVisualArtifactPath(path)
  return page.screenshot({
    path: artifactPath,
    base64: true,
    ...options,
  })
}
