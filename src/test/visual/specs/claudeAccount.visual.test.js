import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import ClaudeAccountHost from '../../../visual-host/hosts/ClaudeAccountHost.svelte'
import { claudeAccountScenarios } from '../fixtures/claudeAccount.fixtures.js'
import { captureVisual, renderVisual } from '../renderVisual.js'

describe('Claude account chooser and chip visual coverage', () => {
  it.each(claudeAccountScenarios)('captures $name', async (scenario) => {
    await renderVisual(ClaudeAccountHost, {
      theme: scenario.theme,
      props: { scenario },
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)

    const chip = screen.queryByTestId('claude-account-chip')
    expect(Boolean(chip)).toBe(scenario.expected.chip)

    const chooser = screen.queryByTestId('claude-account-chooser')
    expect(Boolean(chooser)).toBe(scenario.expected.chooser)

    const screenshotPath = await captureVisual(`claudeAccount/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
