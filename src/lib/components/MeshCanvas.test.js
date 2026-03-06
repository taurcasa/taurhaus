import { describe, it, expect, vi, beforeAll, afterAll } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshCanvas from './MeshCanvas.svelte'

let previousResizeObserver

beforeAll(() => {
  previousResizeObserver = globalThis.ResizeObserver
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterAll(() => {
  if (previousResizeObserver) {
    globalThis.ResizeObserver = previousResizeObserver
    return
  }
  delete globalThis.ResizeObserver
})

const lead = {
  id: 'lead-1',
  name: 'team-lead',
  tool: 'claude',
  model: 'opus',
  status: 'active',
}

function makeAgents(count) {
  return Array.from({ length: count }, (_, index) => ({
    id: `agent-${index + 1}`,
    name: `agent-${index + 1}`,
    tool: index % 2 === 0 ? 'codex' : 'gemini',
    model: 'gpt-5',
    status: index % 3 === 0 ? 'active' : 'idle',
  }))
}

function centerX(element) {
  return Number.parseFloat(element.getAttribute('data-center-x') || '0')
}

function centerY(element) {
  return Number.parseFloat(element.getAttribute('data-center-y') || '0')
}

function connectionXBounds(connectionPath) {
  const raw = String(connectionPath?.getAttribute('d') ?? '')
  const numbers = raw.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? []
  const xValues = [numbers[0], numbers[2], numbers[4], numbers[6]].filter((value) =>
    Number.isFinite(value)
  )
  return {
    min: Math.min(...xValues),
    max: Math.max(...xValues),
  }
}

function connectionStartX(connectionPath) {
  const raw = String(connectionPath?.getAttribute('d') ?? '')
  const numbers = raw.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? []
  return numbers[0]
}

function latestAnchor(mockFn) {
  const anchors = mockFn.mock.calls
    .map(([anchor]) => anchor)
    .filter(Boolean)
  return anchors[anchors.length - 1] ?? null
}

describe('MeshCanvas', () => {
  it('renders lead node when lead prop is provided', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: [],
        mode: 'setup',
      },
    })

    expect(screen.getByTestId('mesh-node-lead')).toBeInTheDocument()
  })

  it('renders correct number of agent nodes', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(4),
        mode: 'setup',
      },
    })

    expect(screen.getAllByTestId('mesh-node-agent')).toHaveLength(4)
  })

  it('positions lead centered in the canvas layout', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(2),
      },
    })

    const leadNode = screen.getByTestId('mesh-node-lead')
    expect(centerX(leadNode)).toBeCloseTo(300, 0)
  })

  it('compresses horizontal spacing for many agents', async () => {
    const { rerender } = render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(3),
      },
    })

    const compactA = screen.getAllByTestId('mesh-node-agent')
    const spacingThree = centerX(compactA[1]) - centerX(compactA[0])

    await rerender({
      lead,
      agents: makeAgents(6),
    })

    const compactB = screen.getAllByTestId('mesh-node-agent')
    const spacingSix = centerX(compactB[1]) - centerX(compactB[0])

    expect(spacingSix).toBeLessThan(spacingThree)
  })

  it('wraps agent rows at 7+ agents', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(7),
      },
    })

    const yValues = new Set(
      screen
        .getAllByTestId('mesh-node-agent')
        .map(node => Math.round(centerY(node)))
    )
    const leadY = Math.round(centerY(screen.getByTestId('mesh-node-lead')))
    const rows = [...yValues].sort((a, b) => a - b)

    expect(yValues.size).toBe(2)
    expect(rows[0]).toBeGreaterThan(leadY + 70)
    expect(rows[1] - rows[0]).toBeGreaterThanOrEqual(80)
  })

  it('renders one connection per agent', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(5),
      },
    })

    expect(screen.getAllByTestId('mesh-connection')).toHaveLength(5)
  })

  it('keeps all 5-6 agent connections within canvas bounds', async () => {
    const { container, rerender } = render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(5),
      },
    })

    for (const count of [5, 6]) {
      await rerender({
        lead,
        agents: makeAgents(count),
      })

      const svg = container.querySelector('svg.mesh-canvas-connections')
      expect(svg).toBeInTheDocument()
      const viewBoxWidth = Number((svg.getAttribute('viewBox') || '0 0 600 0').split(' ')[2])
      for (const connection of screen.getAllByTestId('mesh-connection')) {
        const bounds = connectionXBounds(connection)
        expect(bounds.min).toBeGreaterThanOrEqual(0)
        expect(bounds.max).toBeLessThanOrEqual(viewBoxWidth)
      }
    }
  })

  it('preserves distinct lead-side connection anchors when collapsing from two rows to one', async () => {
    const { rerender } = render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(8),
        mode: 'runtime',
      },
    })

    await rerender({
      lead,
      agents: makeAgents(5),
      mode: 'runtime',
    })

    const agentRows = new Set(
      screen
        .getAllByTestId('mesh-node-agent')
        .map((node) => Math.round(centerY(node)))
    )
    expect(agentRows.size).toBe(1)

    const startXs = screen
      .getAllByTestId('mesh-connection')
      .map((connection) => connectionStartX(connection))

    expect(new Set(startXs).size).toBe(5)
  })

  it('shows add node in setup mode and hides it in runtime mode', async () => {
    const { rerender } = render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(2),
        mode: 'setup',
      },
    })

    expect(screen.getByTestId('mesh-add-node')).toBeInTheDocument()

    await rerender({
      lead,
      agents: makeAgents(2),
      mode: 'runtime',
    })

    expect(screen.queryByTestId('mesh-add-node')).not.toBeInTheDocument()
  })

  it('calls onNodeClick when a node is clicked', async () => {
    const onNodeClick = vi.fn()

    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(1),
        onNodeClick,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    expect(onNodeClick).toHaveBeenCalledWith('agent-1')
  })

  it('calls onAddClick when add node is clicked', async () => {
    const onAddClick = vi.fn()

    render(MeshCanvas, {
      props: {
        lead,
        agents: [],
        mode: 'setup',
        onAddClick,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-add-node'))
    expect(onAddClick).toHaveBeenCalledTimes(1)
  })

  it('emits a clamped top anchor for selected agents', async () => {
    const onDetailAnchorChange = vi.fn()

    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(1),
        mode: 'runtime',
        selectedNodeId: 'agent-1',
        onDetailAnchorChange,
      },
    })

    await waitFor(() => {
      const anchors = onDetailAnchorChange.mock.calls
        .map(([anchor]) => anchor)
        .filter(Boolean)
      expect(anchors.length).toBeGreaterThan(0)
      const anchor = anchors[anchors.length - 1]
      expect(anchor.placement).toBe('top')
      expect(anchor.left).toBeGreaterThanOrEqual(8)
      expect(anchor.top).toBeGreaterThanOrEqual(8)
      expect(anchor.cardWidth).toBeGreaterThanOrEqual(176)
      expect(anchor.cardWidth).toBeLessThanOrEqual(240)
    })
  })

  it('flips detail placement below when selected node is near the top', async () => {
    const onDetailAnchorChange = vi.fn()

    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(2),
        mode: 'runtime',
        selectedNodeId: 'lead-1',
        onDetailAnchorChange,
      },
    })

    await waitFor(() => {
      const anchors = onDetailAnchorChange.mock.calls
        .map(([anchor]) => anchor)
        .filter(Boolean)
      expect(anchors.length).toBeGreaterThan(0)
      expect(anchors[anchors.length - 1].placement).toBe('bottom')
    })
  })

  it('keeps detail anchors clamped within canvas bounds at horizontal and vertical edges', async () => {
    const onDetailAnchorChange = vi.fn()

    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(1),
        mode: 'runtime',
        selectedNodeId: 'agent-1',
        onDetailAnchorChange,
      },
    })

    const canvas = screen.getByTestId('mesh-canvas')
    const node = screen.getByTestId('mesh-node-agent')
    const canvasRect = {
      left: 0,
      top: 0,
      width: 360,
      height: 520,
      right: 360,
      bottom: 520,
    }
    canvas.getBoundingClientRect = () => canvasRect

    node.getBoundingClientRect = () => ({
      left: 2,
      top: 12,
      width: 44,
      height: 64,
      right: 46,
      bottom: 76,
    })
    await fireEvent(window, new Event('resize'))

    await waitFor(() => {
      const anchor = latestAnchor(onDetailAnchorChange)
      expect(anchor).not.toBeNull()
      expect(anchor.left).toBe(8)
      expect(anchor.top).toBeGreaterThanOrEqual(8)
      expect(anchor.top).toBeLessThanOrEqual(288)
      expect(anchor.placement).toBe('bottom')
    })

    node.getBoundingClientRect = () => ({
      left: 330,
      top: 560,
      width: 44,
      height: 64,
      right: 374,
      bottom: 624,
    })
    await fireEvent(window, new Event('resize'))

    await waitFor(() => {
      const anchor = latestAnchor(onDetailAnchorChange)
      expect(anchor).not.toBeNull()
      expect(anchor.left).toBe(112)
      expect(anchor.top).toBe(288)
      expect(anchor.placement).toBe('top')
    })
  })

  it('dismisses selected detail on Escape and outside clicks', async () => {
    const onDismissDetail = vi.fn()

    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(1),
        mode: 'runtime',
        selectedNodeId: 'agent-1',
        onDismissDetail,
      },
    })

    await fireEvent.keyDown(window, { key: 'Escape', code: 'Escape' })
    expect(onDismissDetail).toHaveBeenCalledTimes(1)

    onDismissDetail.mockClear()
    await fireEvent.pointerDown(screen.getByTestId('mesh-node-agent'))
    expect(onDismissDetail).not.toHaveBeenCalled()

    await fireEvent.pointerDown(document.body)
    expect(onDismissDetail).toHaveBeenCalledTimes(1)
  })

  it('handles empty state with no lead', () => {
    render(MeshCanvas, {
      props: {
        lead: null,
        agents: makeAgents(3),
      },
    })

    expect(screen.queryByTestId('mesh-node-lead')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-node-agent')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-connection')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-add-node')).not.toBeInTheDocument()
  })

  it('applies staggered init animation delays in initializing mode', async () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(3),
        mode: 'initializing',
      },
    })

    await waitFor(() => {
      const connections = screen.getAllByTestId('mesh-connection')
      expect(connections[0].getAttribute('style')).toContain('mesh-draw 400ms ease-out 0ms forwards')
      expect(connections[1].getAttribute('style')).toContain('mesh-draw 400ms ease-out 200ms forwards')
      expect(connections[2].getAttribute('style')).toContain('mesh-draw 400ms ease-out 400ms forwards')
    })
  })

  it('maps runtime connection styling to active, idle, and offline statuses', () => {
    render(MeshCanvas, {
      props: {
        lead,
        mode: 'runtime',
        agents: [
          { ...makeAgents(1)[0], id: 'agent-a', status: 'active' },
          { ...makeAgents(1)[0], id: 'agent-b', status: 'idle' },
          { ...makeAgents(1)[0], id: 'agent-c', status: 'offline' },
        ],
      },
    })

    const [activeConnection, idleConnection, offlineConnection] = screen.getAllByTestId('mesh-connection')
    const activeStyle = activeConnection.getAttribute('style') || ''
    const idleStyle = idleConnection.getAttribute('style') || ''
    const offlineStyle = offlineConnection.getAttribute('style') || ''

    expect(activeStyle).toContain('mesh-connection-breathe')
    expect(idleStyle).toContain('opacity: 0.6')
    expect(offlineStyle).toContain('stroke-dasharray: 6,4')
    expect(offlineStyle).toContain('opacity: 0.28')
  })

  it('parses initializing steps from object fields and renders connections', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(3),
        mode: 'initializing',
        initSteps: {
          completedIds: ['agent-1'],
          activeId: 'agent-2',
        },
      },
    })

    expect(screen.getAllByTestId('mesh-connection')).toHaveLength(3)
  })

  it('parses initializing steps from alternate object fields', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(4),
        mode: 'initializing',
        initSteps: {
          completed: ['agent-1'],
          currentId: 'agent-2',
          completedCount: 2,
        },
      },
    })

    expect(screen.getAllByTestId('mesh-connection')).toHaveLength(4)
  })

  it('parses initializing steps from array entries with mixed statuses', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(3),
        mode: 'initializing',
        initSteps: [
          { id: 'agent-1', status: 'succeeded' },
          { id: 'agent-2', status: 'running' },
          { id: 'agent-3', status: 'initializing' },
          'agent-1',
        ],
      },
    })

    expect(screen.getAllByTestId('mesh-connection')).toHaveLength(3)
  })

  it('supports lead/agent cli_tool and model_name fallbacks', () => {
    render(MeshCanvas, {
      props: {
        lead: {
          id: null,
          name: 'lead',
          cli_tool: 'claude',
          model_name: 'opus',
          status: 'active',
        },
        agents: [
          {
            id: null,
            name: 'agent-fallback',
            cli_tool: 'codex',
            model_name: 'gpt-5',
            status: 'idle',
          },
        ],
        mode: 'runtime',
      },
    })

    expect(screen.getByTestId('mesh-node-model-lead')).toHaveTextContent('opus')
    expect(screen.getByTestId('mesh-node-model-agent')).toHaveTextContent('gpt-5')
  })

  it('falls back to setup mode for unknown mode and non-array agents', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: null,
        mode: 'unexpected-mode',
      },
    })

    expect(screen.getByTestId('mesh-node-lead')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-node-agent')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-add-node')).toBeInTheDocument()
  })
})
