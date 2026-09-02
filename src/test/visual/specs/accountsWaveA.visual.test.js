import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import AccountsWaveAHost from '../../../visual-host/hosts/AccountsWaveAHost.svelte'
import { accountsWaveAScenarios } from '../fixtures/accountsWaveA.fixtures.js'
import { captureVisual, renderVisual } from '../renderVisual.js'

describe('Accounts Wave A visual coverage', () => {
  it.each(accountsWaveAScenarios)('captures $name', async (scenario) => {
    await renderVisual(AccountsWaveAHost, {
      theme: scenario.theme,
      props: { scenario },
      viewport: { width: 1100, height: 760 },
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(screen.getByTestId(scenario.expectedTestId)).toBeInTheDocument()

    const screenshotPath = await captureVisual(`accounts-wave-a/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
