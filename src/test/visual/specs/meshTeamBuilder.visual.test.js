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
      viewport: scenario.viewport,
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(document.body.dataset.theme).toBe(scenario.theme)
    expect(screen.getByTestId('mesh-builder-roster')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-catalog')).toHaveAttribute(
      'data-collapsed',
      scenario.expected.catalogCollapsed
    )

    for (const label of scenario.expected.labels) {
      expect(screen.queryAllByText(label).length).toBeGreaterThan(0)
    }
    if (scenario.expected.accountPickerMember) {
      const picker = screen.getByTestId(
        `mesh-builder-account-picker-${scenario.expected.accountPickerMember}`
      )
      expect(picker).toBeInTheDocument()
      const card = picker.closest('article')
      const pickerBounds = picker.getBoundingClientRect()
      const cardBounds = card.getBoundingClientRect()
      expect(getComputedStyle(card).overflow).toBe('visible')
      expect(pickerBounds.left).toBeGreaterThanOrEqual(cardBounds.left)
      expect(pickerBounds.right).toBeLessThanOrEqual(cardBounds.right)
    }

    const screenshotPath = await captureVisual(`meshTeamBuilder/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(`meshTeamBuilder/${scenario.name}.png`)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
