import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshNodeRoleCard from './MeshNodeRoleCard.svelte'

function renderRoleCard(props = {}) {
  return render(MeshNodeRoleCard, {
    props: {
      node: {
        name: 'developer1',
        tool: 'codex',
        model: 'gpt-5.4 high',
        ...props.node,
      },
      dark: true,
      ...(Object.fromEntries(Object.entries(props).filter(([key]) => key !== 'node'))),
    },
  })
}

describe('MeshNodeRoleCard', () => {
  it('renders a compact runtime summary with tool, model, status, and click hint', () => {
    renderRoleCard({
      node: {
        roleName: 'Claude Reviewer',
        status: 'active',
        description: 'Perform code reviews focused on correctness, regressions, security risk, and missing tests.',
        focusArea: '',
        contextSummary: '',
        behaviorSummary: '',
      },
    })

    expect(screen.getByTestId('mesh-node-role-card-role-name')).toHaveTextContent('Claude Reviewer')
    expect(screen.getByTestId('mesh-node-role-card-tool-model')).toHaveTextContent('Codex · gpt-5.4 high')
    expect(screen.getByTestId('mesh-node-role-card-status')).toHaveTextContent('Active')
    expect(screen.getByTestId('mesh-node-role-card-summary')).toHaveTextContent(
      'Perform code reviews focused on correctness, regressions, security risk, and missing tests.'
    )
    expect(screen.getByTestId('mesh-node-role-card-hint')).toHaveTextContent('Click for details')
    expect(screen.queryByTestId('mesh-node-role-card-placeholder')).not.toBeInTheDocument()
  })

  it('renders placeholder content when role fields are empty', () => {
    renderRoleCard({
      node: {
        roleName: '',
        focusArea: '',
        contextSummary: null,
        behaviorSummary: null,
        status: 'idle',
      },
    })

    expect(screen.getByTestId('mesh-node-role-card')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-role-card-name')).toHaveTextContent('developer1')
    expect(screen.getByTestId('mesh-node-role-card-tool-model')).toHaveTextContent('Codex · gpt-5.4 high')
    expect(screen.getByTestId('mesh-node-role-card-status')).toHaveTextContent('Idle')
    expect(screen.getByTestId('mesh-node-role-card-placeholder-title')).toHaveTextContent('No role defined')
    expect(screen.getByTestId('mesh-node-role-card-placeholder-message')).toHaveTextContent(
      'Assign a role template to show a compact focus summary here.'
    )
    expect(screen.getByTestId('mesh-node-role-card-hint')).toHaveTextContent('Click for details')
    expect(screen.queryByTestId('mesh-node-role-card-summary')).not.toBeInTheDocument()
  })

  it('renders placeholder content in both dark and light themes', async () => {
    const view = renderRoleCard({
      node: {
        roleName: '',
        focusArea: '',
        contextSummary: '',
        behaviorSummary: '',
      },
      dark: true,
    })

    expect(screen.getByTestId('mesh-node-role-card').className).toContain('text-zinc-100')

    await view.rerender({
      node: {
        name: 'developer1',
        tool: 'codex',
        model: 'gpt-5.4 high',
        roleName: '',
        focusArea: '',
        contextSummary: '',
        behaviorSummary: '',
      },
      dark: false,
      anchor: null,
    })

    expect(screen.getByTestId('mesh-node-role-card').className).toContain('text-zinc-900')
    expect(screen.getByTestId('mesh-node-role-card-placeholder')).toBeInTheDocument()
  })
})
