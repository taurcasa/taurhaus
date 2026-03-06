import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import {
  empty_noAgents_light,
  meshCanvasScenarios,
  runtime_fiveAgents_dark,
  runtime_fiveAgents_light,
  runtime_threeAgents_dark,
} from '../fixtures/meshCanvas.fixtures.js'
import MeshCanvasVisualHost from './MeshCanvasVisualHost.svelte'
import { captureVisual, renderVisual } from '../renderVisual.js'

describe('MeshCanvas visual pilot', () => {
  it('captures the pilot runtime_threeAgents_dark scenario', async () => {
    await renderVisual(MeshCanvasVisualHost, {
      theme: runtime_threeAgents_dark.theme,
      props: { scenario: runtime_threeAgents_dark },
      ipc: {
        coordinationGetLiveTeamStatus: {
          teamName: 'mesh-visual-pilot',
          leadName: 'team-lead',
          members: runtime_threeAgents_dark.members,
        },
      },
    })

    expect(screen.getByTestId('mesh-node-lead')).toBeInTheDocument()
    expect(screen.getAllByTestId('mesh-node-agent')).toHaveLength(3)
    expect(screen.getAllByTestId('mesh-connection')).toHaveLength(3)
  })

  it.each(meshCanvasScenarios)('captures $name', async (scenario) => {
    await renderVisual(MeshCanvasVisualHost, {
      theme: scenario.theme,
      props: { scenario },
      ipc: {
        coordinationGetLiveTeamStatus: {
          teamName: 'mesh-visual-pilot',
          leadName: 'team-lead',
          members: scenario.members,
        },
      },
    })

    const meshCanvas = screen.getByTestId('mesh-canvas')
    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(document.body.dataset.theme).toBe(scenario.theme)
    expect(meshCanvas.classList.contains('is-light')).toBe(scenario.theme === 'light')

    if (scenario === empty_noAgents_light) {
      expect(screen.queryAllByTestId('mesh-node-agent')).toHaveLength(0)
      expect(screen.queryAllByTestId('mesh-connection')).toHaveLength(0)
      expect(screen.getByTestId('mesh-add-node')).toBeInTheDocument()
    } else {
      expect(screen.getAllByTestId('mesh-node-agent')).toHaveLength(scenario.connections.length)
      expect(screen.getAllByTestId('mesh-connection')).toHaveLength(scenario.connections.length)
    }

    if (scenario === runtime_fiveAgents_dark || scenario === runtime_fiveAgents_light) {
      expect(screen.getByText('[mesh]')).toBeInTheDocument()
    }

    const screenshotPath = await captureVisual(`meshCanvas/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(`meshCanvas/${scenario.name}.png`)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(960)
    expect(artifact.height).toBe(640)
  })
})
