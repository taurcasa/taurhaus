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
    expect(statusDot.getAttribute('style')).not.toContain('var(--color-warning-500)')
  })

  it('marks stopped nodes with neutral offline styling', () => {
    render(MeshNode, {
      props: {
        status: 'offline',
      },
    })

    const node = document.querySelector('[data-testid="mesh-node-agent"]')
    expect(node.classList.contains('is-offline')).toBe(true)
  })

  it('renders MeshNode styles with tokenized color variables', () => {
    const source = readFileSync(`${process.cwd()}/src/lib/components/MeshNode.svelte`, 'utf8')
    expect(source).toContain('var(--mesh-node-border-dark)')
    expect(source).toContain('var(--mesh-node-bg-dark)')
    expect(source).toContain('var(--mesh-node-text-dark)')
    expect(source).toContain('var(--mesh-node-status-shadow-light)')
    expect(source).toContain('.mesh-node:focus-visible')
    expect(source).not.toMatch(/#[0-9A-Fa-f]{3,8}/)
    expect(source).not.toMatch(/\brgba?\(/)
  })

  it('shows the assignment effort beside the model, with the reason on hover', async () => {
    const view = render(MeshNode, {
      props: {
        role: 'agent',
        model: 'gpt-5.6-terra',
        taskEffort: 'high',
        taskEffortWhy: 'the migration is irreversible',
      },
    })

    const chip = screen.getByTestId('mesh-node-task-effort-agent')
    expect(chip).toHaveTextContent('high')
    expect(chip).toHaveAttribute('title', 'Task effort: high — the migration is irreversible')
    expect(chip.parentElement).toHaveAttribute('data-testid', 'mesh-node-meta-row-agent')

    await view.rerender({
      role: 'agent',
      model: 'gpt-5.6-terra',
      taskEffort: 'high',
      taskEffortWhy: '',
    })
    expect(screen.getByTestId('mesh-node-task-effort-agent')).toHaveAttribute(
      'title',
      'Task effort: high'
    )
  })

  it('shows the launch effort beside the assignment effort so the two can be compared', () => {
    render(MeshNode, {
      props: {
        role: 'agent',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'medium',
        taskEffort: 'high',
        taskEffortWhy: 'the migration is irreversible',
      },
    })

    const launch = screen.getByTestId('mesh-node-launch-effort-agent')
    expect(launch).toHaveTextContent('medium')
    expect(launch).toHaveAttribute('title', 'Launch effort: medium')
    expect(screen.getByTestId('mesh-node-task-effort-agent')).toHaveTextContent('high')
  })

  it('shows no launch effort chip for a member launched without one', () => {
    render(MeshNode, { props: { role: 'agent', model: 'gpt-5.6-terra', taskEffort: 'high' } })

    expect(screen.queryByTestId('mesh-node-launch-effort-agent')).toBeNull()
  })

  it('shows no effort chip for a member with no assignment effort', () => {
    render(MeshNode, { props: { role: 'agent', model: 'gpt-5.6-terra' } })

    expect(screen.queryByTestId('mesh-node-task-effort-agent')).toBeNull()
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

  it('shows one account line for applied, not-guaranteed, and fallback states', async () => {
    // Regression: commit 0f2bfbb0 kept the opaque-base warning on app launch
    // results only, so managed member nodes hid the same uncertainty.
    const view = render(MeshNode, {
      props: {
        role: 'agent',
        accountLabel: 'Personal',
        accountApplied: false,
        accountNote: 'opaque_base_command',
        accountNoteDetail: 'team-wrapper',
        height: 82,
      },
    })

    expect(screen.getByTestId('mesh-node-account-line-agent')).toHaveTextContent(
      'Personal · not guaranteed'
    )
    expect(screen.getByTestId('mesh-node-account-line-agent')).toHaveAttribute(
      'title',
      'taurhaus could not select an account: your launch command runs "team-wrapper", which is not the Claude CLI'
    )
    expect(screen.getByTestId('mesh-node-agent')).toHaveAttribute('data-node-height', '82')

    await view.rerender({
      role: 'agent',
      accountLabel: 'Personal',
      accountApplied: false,
      accountNote: 'account_fallback',
      accountFallbackFrom: 'Work',
    })
    expect(screen.getByTestId('mesh-node-account-line-agent')).toHaveTextContent(
      'was Work → now Personal'
    )

    await view.rerender({
      role: 'agent',
      accountLabel: 'Work',
      accountApplied: true,
      accountNote: '',
      accountFallbackFrom: '',
    })
    expect(screen.getByTestId('mesh-node-account-line-agent')).toHaveTextContent('Work · applied')
    expect(screen.getByTestId('mesh-node-account-meter-agent')).toBeInTheDocument()
  })
})
