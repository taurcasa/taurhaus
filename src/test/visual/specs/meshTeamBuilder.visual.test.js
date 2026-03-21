import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import MeshTeamBuilderHost from '../../../visual-host/hosts/MeshTeamBuilderHost.svelte'
import { meshTeamBuilderScenarios } from '../fixtures/meshTeamBuilder.fixtures.js'
import { captureVisual, renderVisual } from '../renderVisual.js'

describe('MeshTeamBuilder visual coverage', () => {
  it.each(meshTeamBuilderScenarios)('captures $name', async (scenario) => {
    await renderVisual(MeshTeamBuilderHost, {
      theme: scenario.theme,
      props: { scenario },
      viewport: { width: 1100, height: 1200 },
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(document.body.dataset.theme).toBe(scenario.theme)
    expect(screen.getByTestId('mesh-builder-roster')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-catalog')).toHaveAttribute(
      'data-collapsed',
      scenario.expected.catalogCollapsed
    )

    for (const label of scenario.expected.labels) {
      expect(screen.getByText(label)).toBeInTheDocument()
    }

    const screenshotPath = await captureVisual(`meshTeamBuilder/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(`meshTeamBuilder/${scenario.name}.png`)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(1100)
    expect(artifact.height).toBe(1200)
  })
})
