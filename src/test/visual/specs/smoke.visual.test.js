import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import VisualSmokeFixture from '../fixtures/VisualSmokeFixture.svelte'
import { visualIpcMocks } from '../ipcVisualMocks.js'
import { captureVisual, captureVisualBase64, renderVisual } from '../renderVisual.js'

describe('visual smoke infrastructure', () => {
  it.each([
    { theme: 'light', file: 'smoke/smoke-light.png' },
    { theme: 'dark', file: 'smoke/smoke-dark.png' },
  ])('captures a $theme screenshot with deterministic viewport sizing', async ({ theme, file }) => {
    const viewport = { width: 960, height: 640 }

    visualIpcMocks.getProject('leaked-call')
    expect(visualIpcMocks.getProject).toHaveBeenCalledTimes(1)

    await renderVisual(VisualSmokeFixture, {
      theme,
      viewport,
      props: {
        label: `${theme} mode smoke`,
      },
    })

    expect(visualIpcMocks.getProject).not.toHaveBeenCalled()
    expect(document.documentElement.dataset.theme).toBe(theme)
    expect(document.body.dataset.theme).toBe(theme)

    const screenshotPath = await captureVisual(file)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(file)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(viewport.width)
    expect(artifact.height).toBe(viewport.height)

    const inlinePath = file.replace('.png', '-inline.png')
    const base64Result = await captureVisualBase64(inlinePath)
    expect(base64Result.base64.length).toBeGreaterThan(100)
  })
})
