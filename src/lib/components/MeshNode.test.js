import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'
import MeshNode from './MeshNode.svelte'

describe('MeshNode', () => {
  it('applies role and theme classes for node variants', async () => {
    const { rerender } = render(MeshNode, {
      props: {
        nodeId: 'lead-1',
        name: 'team-lead',
        role: 'lead',
        selected: true,
        dark: true,
      },
    })

    const leadNode = document.querySelector('[data-testid="mesh-node-lead"]')
    expect(leadNode).toBeInTheDocument()
    expect(leadNode.classList.contains('is-lead')).toBe(true)
    expect(leadNode.classList.contains('is-selected')).toBe(true)
    expect(leadNode.classList.contains('is-light')).toBe(false)

    await rerender({
      nodeId: 'agent-1',
      name: 'agent',
      role: 'agent',
      selected: false,
      dark: false,
    })

    const agentNode = document.querySelector('[data-testid="mesh-node-agent"]')
    expect(agentNode).toBeInTheDocument()
    expect(agentNode.classList.contains('is-lead')).toBe(false)
    expect(agentNode.classList.contains('is-selected')).toBe(false)
    expect(agentNode.classList.contains('is-light')).toBe(true)
  })

  it('uses status token variables instead of literal colors', async () => {
    const { rerender } = render(MeshNode, {
      props: { status: 'active' },
    })

    let statusDot = document.querySelector('.mesh-node-status')
    expect(statusDot.getAttribute('style')).toContain('var(--color-success-500)')

    await rerender({ status: 'idle' })
    statusDot = document.querySelector('.mesh-node-status')
    expect(statusDot.getAttribute('style')).toContain('var(--color-warning-500)')

    await rerender({ status: 'offline' })
    statusDot = document.querySelector('.mesh-node-status')
    expect(statusDot.getAttribute('style')).toContain('var(--mesh-node-status-offline)')
  })

  it('renders MeshNode styles with tokenized color variables', () => {
    const source = readFileSync(`${process.cwd()}/src/lib/components/MeshNode.svelte`, 'utf8')
    expect(source).toContain('var(--mesh-node-border-dark)')
    expect(source).toContain('var(--mesh-node-bg-dark)')
    expect(source).toContain('var(--mesh-node-text-dark)')
    expect(source).toContain('var(--mesh-node-status-shadow-light)')
    expect(source).not.toMatch(/#[0-9A-Fa-f]{3,8}/)
    expect(source).not.toMatch(/\brgba?\(/)
  })

  it('shows a compact project chip only for cross-project members', async () => {
    const view = render(MeshNode, {
      props: {
        role: 'agent',
        model: 'gpt-5.4 high',
        isCrossProject: true,
        projectLabel: 'mesh',
      },
    })

    const chip = screen.getByTestId('mesh-node-project-chip-agent')
    expect(chip).toHaveTextContent('[mesh]')
    expect(chip.parentElement).toHaveAttribute('data-testid', 'mesh-node-meta-row-agent')
    expect(screen.getByTestId('mesh-node-model-agent').parentElement).toBe(chip.parentElement)

    await view.rerender({
      role: 'agent',
      isCrossProject: false,
      projectLabel: '',
    })

    expect(screen.queryByTestId('mesh-node-project-chip-agent')).not.toBeInTheDocument()
  })
})
