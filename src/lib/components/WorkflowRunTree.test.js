import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import WorkflowRunTree from './WorkflowRunTree.svelte'

const BOX = { left: 120, top: 240, width: 200, height: 100 }

function agent(overrides = {}) {
  return {
    agent_id: 'agent-1',
    label: null,
    phase: null,
    model: 'claude-opus-5',
    state: 'running',
    prompt_preview: 'Implement the feature',
    last_tool: 'Bash',
    tokens: 8434,
    tool_calls: 3,
    last_write_at: 1787949436814,
    ...overrides,
  }
}

function liveRun(overrides = {}) {
  return {
    run_id: 'wf_live',
    name: 'feature-pr',
    phases: ['Implement', 'Review'],
    status: 'live',
    started_at: 1787949435335,
    finished_at: null,
    agents: [agent()],
    totals: { agents: 3, done: 1, tokens: 8434, tool_calls: 3, duration_ms: null },
    ...overrides,
  }
}

function finishedRun(overrides = {}) {
  return {
    run_id: 'wf_done',
    name: 'docs-sweep',
    phases: ['Sweep'],
    status: 'completed',
    started_at: 1000,
    finished_at: 139_000,
    agents: [agent({ state: 'done' })],
    totals: { agents: 2, done: 2, tokens: 12_400, tool_calls: 9, duration_ms: 138_000 },
    ...overrides,
  }
}

function renderTree(props = {}) {
  return render(WorkflowRunTree, {
    props: { runs: [liveRun()], box: BOX, ...props },
  })
}

describe('WorkflowRunTree', () => {
  it('places itself in the box the layout engine gave it', () => {
    renderTree()

    const tree = screen.getByTestId('workflow-run-tree')
    expect(tree.style.left).toBe('120px')
    expect(tree.style.top).toBe('240px')
    expect(tree.style.width).toBe('200px')
    expect(tree.style.height).toBe('100px')
  })

  it('heads each run with its name and totals', () => {
    renderTree()

    expect(screen.getByTestId('workflow-run-header')).toHaveTextContent(
      'feature-pr · 1/3 done · 8.4k tokens'
    )
  })

  it('expands a live run into its phase and agent rows', () => {
    renderTree({
      runs: [liveRun({
        agents: [
          agent({ agent_id: 'a', phase: 'Implement', label: 'implementer' }),
          agent({ agent_id: 'b', phase: 'Review', label: 'reviewer', state: 'queued' }),
        ],
      })],
    })

    expect(screen.getAllByTestId('workflow-run-phase').map((row) => row.textContent.trim()))
      .toEqual(['Implement', 'Review'])

    const agents = screen.getAllByTestId('workflow-run-agent')
    expect(agents).toHaveLength(2)
    expect(agents[0]).toHaveTextContent('implementer')
    expect(agents[0]).toHaveTextContent('claude-opus-5')
    expect(agents[0]).toHaveTextContent('Bash')
    expect(agents[0]).toHaveTextContent('8.4k')
    expect(agents[0].dataset.state).toBe('running')
    expect(agents[1].dataset.state).toBe('queued')
  })

  it('renders no phase row for agents the scanner could not place', () => {
    renderTree()

    expect(screen.queryAllByTestId('workflow-run-phase')).toHaveLength(0)
    expect(screen.getByTestId('workflow-run-agent')).toHaveTextContent('Implement the feature')
  })

  it('collapses a finished run to one line', () => {
    renderTree({ runs: [finishedRun()] })

    expect(screen.getByTestId('workflow-run-header')).toHaveTextContent(
      'docs-sweep · 2/2 done · 12.4k tokens · 2m 18s'
    )
    expect(screen.queryAllByTestId('workflow-run-agent')).toHaveLength(0)
  })

  it('marks a failed run', () => {
    renderTree({ runs: [finishedRun({ status: 'failed' })] })

    expect(screen.getByTestId('workflow-run-header').dataset.status).toBe('failed')
  })

  it('hides the rows of a live run the viewer collapsed', () => {
    renderTree({ collapsedRunIds: ['wf_live'] })

    expect(screen.queryAllByTestId('workflow-run-agent')).toHaveLength(0)
    expect(screen.getByTestId('workflow-run-header')).toBeInTheDocument()
  })

  it('reports a header click as a toggle for that run', async () => {
    const onToggleRun = vi.fn()
    renderTree({ onToggleRun })

    await fireEvent.click(screen.getByTestId('workflow-run-header'))

    expect(onToggleRun).toHaveBeenCalledExactlyOnceWith('wf_live')
  })

  it('renders nothing without a box or without runs', () => {
    const { unmount } = renderTree({ box: null })
    expect(screen.queryByTestId('workflow-run-tree')).not.toBeInTheDocument()
    unmount()

    renderTree({ runs: [] })
    expect(screen.queryByTestId('workflow-run-tree')).not.toBeInTheDocument()
  })

  it('stacks several runs in one box', () => {
    renderTree({ runs: [liveRun(), finishedRun()] })

    expect(screen.getAllByTestId('workflow-run-header')).toHaveLength(2)
    expect(screen.getAllByTestId('workflow-run-agent')).toHaveLength(1)
  })
})
