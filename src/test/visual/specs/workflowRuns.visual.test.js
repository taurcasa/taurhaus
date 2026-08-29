import { fireEvent, screen, waitFor } from '@testing-library/svelte'
import { beforeAll, describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

// The mock registration has to run before the panel's module graph is
// evaluated, so the host is imported dynamically once it is in place.
import '../ipcVisualMocks.js'
import { captureVisual, renderVisual } from '../renderVisual.js'
import { workflowRunsScenarios } from '../fixtures/workflowRuns.fixtures.js'

let WorkflowRunsVisualHost

beforeAll(async () => {
  WorkflowRunsVisualHost = (await import('./WorkflowRunsVisualHost.svelte')).default
})

describe('Workflow run history visual coverage', () => {
  it.each(workflowRunsScenarios)('captures $name', async (scenario) => {
    await renderVisual(WorkflowRunsVisualHost, {
      theme: scenario.theme,
      props: { scenario },
      ipc: scenario.ipc,
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)

    if (scenario.emptyNote) {
      await waitFor(() => {
        expect(screen.getByTestId('workflow-runs-empty-note')).toBeInTheDocument()
      })
      expect(screen.queryByTestId('overview-workflow-runs')).not.toBeInTheDocument()
    } else {
      await waitFor(() => {
        expect(screen.getAllByTestId('workflow-run-row').length).toBeGreaterThan(0)
      })
    }

    if (scenario.selectRunName) {
      const row = screen
        .getAllByTestId('workflow-run-row')
        .find((candidate) => candidate.textContent.includes(scenario.selectRunName))
      expect(row).toBeTruthy()
      await fireEvent.click(row)

      await waitFor(() => {
        expect(screen.getByTestId('workflow-run-detail')).toBeInTheDocument()
      })
      const copyButton = screen.getByTestId('workflow-copy-ledger')
      await waitFor(() => {
        expect(copyButton.disabled).toBe(!scenario.ipc.workflowLedgerRow)
      })
    }

    const screenshotPath = await captureVisual(`workflowRuns/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(`workflowRuns/${scenario.name}.png`)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(960)
    expect(artifact.height).toBe(640)
  })
})
