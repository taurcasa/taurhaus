import { screen, waitFor } from '@testing-library/svelte'
import { beforeAll, describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import '../ipcVisualMocks.js'
import { captureVisualBase64, renderVisual } from '../renderVisual.js'
import { hoverCardScenarios } from '../fixtures/hoverCard.fixtures.js'

let HoverCard

beforeAll(async () => {
  const module = await import('../../../lib/HoverCard.svelte')
  HoverCard = module.default
})

function createAnchorEl(rect) {
  return {
    getBoundingClientRect() {
      return {
        left: rect.left,
        right: rect.right,
        top: rect.top,
        bottom: rect.top + rect.height,
        width: rect.width ?? rect.right - rect.left,
        height: rect.height,
        x: rect.left,
        y: rect.top,
        toJSON() {},
      }
    },
  }
}

const screenshotsByScenario = new Map()
const viewport = { width: 960, height: 640 }
const viewportClip = { x: 0, y: 0, width: viewport.width, height: viewport.height }

describe('HoverCard visual coverage', () => {
  it.each(hoverCardScenarios)('captures $name', async (scenario) => {
    const screenshotPath = `hoverCard/${scenario.name}.png`

    await renderVisual(HoverCard, {
      theme: scenario.theme,
      viewport,
      props: {
        project: scenario.project,
        sessions: scenario.sessions,
        anchorEl: createAnchorEl(scenario.anchorRect),
      },
      ipc: scenario.ipc,
    })

    document.documentElement.style.height = `${viewport.height}px`
    document.body.style.height = `${viewport.height}px`
    document.body.style.overflow = 'hidden'

    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toBeInTheDocument()
      expect(screen.getByText(scenario.expected.verdict)).toBeInTheDocument()
      expect(screen.getByText(scenario.expected.motion)).toBeInTheDocument()
      expect(screen.getByText(scenario.expected.latestChange)).toBeInTheDocument()
    })

    const tooltip = screen.getByRole('tooltip')
    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(document.body.dataset.theme).toBe(scenario.theme)

    if (scenario.theme === 'dark') {
      expect(tooltip.className).toContain('bg-brand-950/96')
    } else {
      expect(tooltip.className).toContain('bg-white/96')
    }

    if (scenario.expected.unresolved) {
      expect(screen.getByText(scenario.expected.unresolved)).toBeInTheDocument()
      expect(screen.getByTestId('hovercard-unresolved')).toBeInTheDocument()
    } else {
      expect(screen.queryByTestId('hovercard-unresolved')).not.toBeInTheDocument()
    }

    if (scenario.expected.relationshipChip) {
      expect(screen.getByText(scenario.expected.relationshipChip)).toBeInTheDocument()
      expect(screen.getByTestId('hovercard-relationship')).toBeInTheDocument()
    } else {
      expect(screen.queryByTestId('hovercard-relationship')).not.toBeInTheDocument()
    }

    const screenshotResult = await captureVisualBase64(screenshotPath, { clip: viewportClip })
    const artifact = await commands.readVisualArtifact(screenshotPath)
    const screenshotBase64 = screenshotResult.base64

    screenshotsByScenario.set(scenario.name, screenshotBase64)

    expect(artifact.path.endsWith(screenshotPath)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(960)
    expect(artifact.height).toBe(640)

    if (scenario.compareAgainst) {
      const baseline = screenshotsByScenario.get(scenario.compareAgainst)
      expect(baseline).toBeTruthy()
      expect(screenshotBase64).not.toBe(baseline)
    }
  })
})
