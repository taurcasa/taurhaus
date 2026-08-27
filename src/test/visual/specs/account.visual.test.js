import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import AccountHost from '../../../visual-host/hosts/AccountHost.svelte'
import { accountScenarios } from '../fixtures/account.fixtures.js'
import { captureVisual, renderVisual } from '../renderVisual.js'

describe('account chooser, chip, and usage visual coverage', () => {
  it.each(accountScenarios)('captures $name', async (scenario) => {
    await renderVisual(AccountHost, {
      theme: scenario.theme,
      props: { scenario },
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)

    const chip = screen.queryByTestId('account-chip')
    expect(Boolean(chip)).toBe(scenario.expected.chip)

    const chooser = screen.queryByTestId('account-chooser')
    expect(Boolean(chooser)).toBe(scenario.expected.chooser)

    const screenshotPath = await captureVisual(`account/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
