import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import ShellPopupsHost from '../../../visual-host/hosts/ShellPopupsHost.svelte'
import { shellPopupsScenarios } from '../fixtures/shellPopups.fixtures.js'
import { captureVisual, renderVisual } from '../renderVisual.js'

describe('Account popups inside the shell markup', () => {
  it.each(shellPopupsScenarios)('captures $name', async (scenario) => {
    await renderVisual(ShellPopupsHost, {
      theme: scenario.theme,
      props: { scenario, theme: scenario.theme },
    })

    const screenshotPath = await captureVisual(`shellPopups/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })

  // Regression: c982822 mounted the chooser in a wrapper inside `.shell-frame`,
  // where `.shell-frame > :not([data-shell-overlay])` overrode `fixed` and the
  // dialog became the last row of the frame's flex column — at the bottom of
  // the window, half cut off. The fixture that would have caught it kept a
  // wrapper of its own (74c7761), which put the overlay one level down and hid
  // the very rule under test.
  it('gives the chooser one overlay, fixed, directly inside the frame', async () => {
    const scenario = shellPopupsScenarios.find((candidate) => candidate.surface === 'chooser')

    await renderVisual(ShellPopupsHost, {
      theme: scenario.theme,
      props: { scenario, theme: scenario.theme },
    })

    const overlays = screen.getAllByTestId('account-chooser-overlay')
    expect(overlays).toHaveLength(1)

    const [overlay] = overlays
    expect(overlay.parentElement.classList.contains('shell-frame')).toBe(true)
    expect(overlay.hasAttribute('data-shell-overlay')).toBe(true)
    expect(getComputedStyle(overlay).position).toBe('fixed')

    // Fixed means the viewport, not the frame's flex column: the dialog is
    // centred in the window rather than sitting under everything else.
    const dialog = screen.getByTestId('account-chooser').getBoundingClientRect()
    expect(dialog.top).toBeGreaterThan(0)
    expect(dialog.bottom).toBeLessThanOrEqual(window.innerHeight)
  })
})
