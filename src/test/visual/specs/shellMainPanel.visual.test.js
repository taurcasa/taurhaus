import { screen, waitFor } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import ShellMainPanel from '../../../lib/components/shell/ShellMainPanel.svelte'
import { captureVisualBase64, renderVisual } from '../renderVisual.js'

const viewport = { width: 1280, height: 760 }
const viewportClip = { x: 0, y: 0, width: viewport.width, height: viewport.height }

describe('ShellMainPanel visual coverage', () => {
  it('captures the stalled daemon recovery banner with degraded project details', async () => {
    const screenshotPath = 'shell/shell-recovery-and-degraded-load.png'

    await renderVisual(ShellMainPanel, {
      theme: 'dark',
      viewport,
      props: {
        daemonStatus: 'disconnected',
        daemonRecoveryEscalated: true,
        projectLoadIssues: [
          { section: 'Recent commits', message: 'boom' },
          { section: 'README', message: 'boom' },
        ],
      },
    })

    document.documentElement.style.height = `${viewport.height}px`
    document.body.style.height = `${viewport.height}px`
    document.body.style.overflow = 'hidden'

    await waitFor(() => {
      expect(screen.getByTestId('daemon-connecting-banner')).toBeInTheDocument()
      expect(screen.getByTestId('daemon-restart-button')).toBeInTheDocument()
      expect(screen.getByTestId('project-load-degraded-banner')).toBeInTheDocument()
    })

    const screenshotResult = await captureVisualBase64(screenshotPath, { clip: viewportClip })
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(screenshotResult.base64).toBeTruthy()
    expect(artifact.path.endsWith(screenshotPath)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBeGreaterThan(0)
    expect(artifact.height).toBeGreaterThan(0)
  })
})
