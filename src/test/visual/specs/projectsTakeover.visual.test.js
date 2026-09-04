import { fireEvent, screen, waitFor } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import ProjectsTakeover from '../../../lib/ProjectsTakeover.svelte'
import { MOCK_PROJECTS } from '../../../lib/ipc/mocks/base.js'
import { captureVisual, renderVisual } from '../renderVisual.js'

// In browser mode the real IPC layer answers with its deterministic mock
// fallbacks (MOCK_PROJECTS for the registered list, an empty scan), so the
// scenarios assert against that data rather than per-test overrides.
describe('Projects takeover visual coverage', () => {
  it.each([
    { theme: 'dark', name: 'doorway_registered_dark' },
    { theme: 'light', name: 'doorway_registered_light' },
  ])('captures the doorway and registered list ($theme)', async ({ theme, name }) => {
    await renderVisual(ProjectsTakeover, {
      theme,
      viewport: { width: 960, height: 720 },
    })

    const doorway = screen.getByTestId('surface-doorway')
    expect(doorway).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Projects' })).toBeInTheDocument()
    expect(screen.getByTestId('projects-back')).toHaveTextContent('Back')
    expect(doorway.querySelector('kbd')).toHaveTextContent('Esc')

    await waitFor(() => {
      expect(screen.getByTestId('registered-list')).toBeInTheDocument()
      expect(screen.getByTestId('projects-registered-count')).toHaveTextContent(
        `${MOCK_PROJECTS.length} registered`
      )
    })

    const screenshotPath = await captureVisual(`projectsTakeover/${name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })

  it.each([
    { theme: 'dark', name: 'add_section_dark' },
    { theme: 'light', name: 'add_section_light' },
  ])('captures the add section with the segmented control ($theme)', async ({ theme, name }) => {
    await renderVisual(ProjectsTakeover, {
      theme,
      viewport: { width: 960, height: 720 },
    })

    await waitFor(() => {
      expect(screen.getByTestId('show-add-section')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('show-add-section'))

    // The browser-mode scan fallback is empty, which lands on the
    // deterministic empty-scan state beneath the segmented control.
    await waitFor(() => {
      expect(screen.getByTestId('empty-scan')).toBeInTheDocument()
    })
    // The three workflows survive as a segmented control.
    expect(screen.getByTestId('mode-scan')).toBeInTheDocument()
    expect(screen.getByTestId('mode-manual')).toBeInTheDocument()
    expect(screen.getByTestId('mode-create')).toBeInTheDocument()

    const screenshotPath = await captureVisual(`projectsTakeover/${name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
