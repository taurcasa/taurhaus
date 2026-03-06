import { screen } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import {
  active_claude_dark,
  cross_project_gemini_dark,
  cross_project_gemini_light,
  meshNodeDetailScenarios,
  noSession_dark,
} from '../fixtures/meshNodeDetail.fixtures.js'
import { captureVisual, renderVisual } from '../renderVisual.js'
import MeshNodeDetailVisualHost from './MeshNodeDetailVisualHost.svelte'

describe('MeshNodeDetail visual spec', () => {
  it('renders the initial active_claude_dark scenario cleanly', async () => {
    await renderVisual(MeshNodeDetailVisualHost, {
      theme: active_claude_dark.theme,
      props: { scenario: active_claude_dark },
    })

    expect(screen.getByTestId('mesh-node-detail-name')).toHaveTextContent('architect-alpha')
    expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
    expect(screen.getByTestId('mesh-node-detail-tool-model')).toHaveTextContent('Claude')
    expect(screen.getByTestId('mesh-node-detail-focus')).toBeEnabled()
  })

  it.each(meshNodeDetailScenarios)('captures $name', async (scenario) => {
    await renderVisual(MeshNodeDetailVisualHost, {
      theme: scenario.theme,
      props: { scenario },
    })

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(document.body.dataset.theme).toBe(scenario.theme)
    expect(screen.getByTestId('mesh-node-detail')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent(
      scenario.member.status === 'active'
        ? 'Active'
        : scenario.member.status === 'idle'
          ? 'Idle'
          : 'Offline'
    )

    if (scenario.mode === 'runtime') {
      expect(screen.getByTestId('mesh-node-detail-capture')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-node-detail-focus')).toBeInTheDocument()
      if (scenario.focusEnabled) {
        expect(screen.getByTestId('mesh-node-detail-focus')).toBeEnabled()
      } else {
        expect(screen.getByTestId('mesh-node-detail-focus')).toBeDisabled()
      }
    } else {
      expect(screen.getByTestId('mesh-node-detail-edit')).toBeInTheDocument()
    }

    if (scenario === noSession_dark) {
      expect(screen.queryByTestId('mesh-node-detail-runtime')).not.toBeInTheDocument()
    } else {
      expect(screen.getByTestId('mesh-node-detail-runtime')).toBeInTheDocument()
    }

    if (scenario === cross_project_gemini_dark || scenario === cross_project_gemini_light) {
      expect(screen.getByTestId('mesh-node-detail-project')).toHaveTextContent('Project: mesh')
      expect(screen.getByTestId('mesh-node-detail-project-context')).toHaveTextContent('/home/user/projects/mesh')
      expect(screen.getByTestId('mesh-node-detail-location')).toHaveTextContent('Location: other project')
    }

    const screenshotPath = await captureVisual(`meshNodeDetail/${scenario.name}.png`)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(`meshNodeDetail/${scenario.name}.png`)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(960)
    expect(artifact.height).toBe(640)
  })
})
