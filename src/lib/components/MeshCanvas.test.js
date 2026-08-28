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
    tool: index % 2 === 0 ? 'codex' : 'agy',
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

function connectionControlXs(connectionPath) {
  const raw = String(connectionPath?.getAttribute('d') ?? '')
  const numbers = raw.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? []
  return {
    start: numbers[0],
    control1: numbers[2],
    control2: numbers[4],
    end: numbers[6],
  }
}

function pathEndpoints(connectionPath) {
  const raw = String(connectionPath?.getAttribute('d') ?? '')
  const numbers = raw.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? []
  return {
    startX: numbers[0],
    startY: numbers[1],
    endX: numbers.at(-2),
    endY: numbers.at(-1),
  }
}

function pathCommands(connectionPath) {
  const raw = String(connectionPath?.getAttribute('d') ?? '')
  return raw.match(/[A-Za-z]/g) ?? []
}

function latestAnchor(mockFn) {
  const anchors = mockFn.mock.calls
    .map(([anchor]) => anchor)
    .filter(Boolean)
  return anchors[anchors.length - 1] ?? null
}

describe('MeshCanvas', () => {
  it.each([1, 3, 5, 8])('renders visible connection paths for %i agents', (count) => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(count),
        mode: 'runtime',
      },
    })

    const connections = screen.getAllByTestId('mesh-connection')
    expect(connections).toHaveLength(count)

    for (const connection of connections) {
      expect(connection.getAttribute('d')).toMatch(/^M\s/)
    }
  })

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

  it('passes cross-project rendering flags to the target agent node and connection', () => {
    const agents = makeAgents(2)
    agents[1] = {
      ...agents[1],
      name: 'mesh-expert',
      status: 'active',
      isCrossProject: true,
      projectLabel: 'mesh',
    }

    render(MeshCanvas, {
      props: {
        lead,
        agents,
        mode: 'runtime',
      },
    })

    const connections = screen.getAllByTestId('mesh-connection')
    const crossProjectConnection = connections[1]
    const chip = screen.getByText('[mesh]')

    expect(chip).toBeInTheDocument()
    expect(crossProjectConnection.getAttribute('style') || '').toContain('opacity: 0.8')
    expect(crossProjectConnection.getAttribute('style') || '').toContain('stroke-dasharray: 6,4')
  })

  it('renders exactly five non-empty connection paths for the 5-agent runtime layout', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(5),
        mode: 'runtime',
      },
    })

    const connections = screen.getAllByTestId('mesh-connection')
    expect(connections).toHaveLength(5)

    for (const connection of connections) {
      expect(String(connection.getAttribute('d') ?? '').trim().length).toBeGreaterThan(0)
    }
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

  it('keeps the correct connection count after removing the center agent and rerendering', async () => {
    const { rerender } = render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(5),
        mode: 'runtime',
      },
    })

    expect(screen.getAllByTestId('mesh-connection')).toHaveLength(5)

    await rerender({
      lead,
      agents: [makeAgents(5)[0], makeAgents(5)[1], makeAgents(5)[3], makeAgents(5)[4]],
      mode: 'runtime',
    })

    const remainingConnections = screen.getAllByTestId('mesh-connection')
    expect(remainingConnections).toHaveLength(4)
    for (const connection of remainingConnections) {
      expect(connection.getAttribute('d')).toMatch(/^M\s/)
    }
  })

  it('fans 5-agent runtime connections outward instead of keeping all bezier control points on the same vertical rails', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: makeAgents(5),
        mode: 'runtime',
      },
    })

    const controls = screen
      .getAllByTestId('mesh-connection')
      .map((connection) => connectionControlXs(connection))

    expect(controls).toHaveLength(5)

    expect(controls[0].control1).toBeLessThan(controls[0].start)
    expect(controls[1].control1).toBeLessThan(controls[1].start)
    expect(controls[3].control1).toBeGreaterThan(controls[3].start)
    expect(controls[4].control1).toBeGreaterThan(controls[4].start)

    expect(controls[0].control2).toBeLessThan(controls[0].end)
    expect(controls[1].control2).toBeLessThan(controls[1].end)
    expect(controls[3].control2).toBeGreaterThan(controls[3].end)
    expect(controls[4].control2).toBeGreaterThan(controls[4].end)

    expect(controls[0].control1).toBeLessThan(controls[1].control1)
    expect(controls[1].control1).toBeLessThan(controls[2].control1)
    expect(controls[2].control1).toBeLessThan(controls[3].control1)
    expect(controls[3].control1).toBeLessThan(controls[4].control1)
  })

  it('fans the center 5-agent runtime connection with a visible bend instead of a hidden vertical line', () => {
    render(MeshCanvas, {
      props: {
        lead,
        agents: [
          { id: 'architect', name: 'architect', tool: 'codex', model: 'gpt-5', status: 'active' },
          { id: 'developer1', name: 'developer1', tool: 'codex', model: 'gpt-5', status: 'active' },
          { id: 'developer2', name: 'developer2', tool: 'codex', model: 'gpt-5', status: 'active' },
          { id: 'developer3', name: 'developer3', tool: 'codex', model: 'gpt-5', status: 'active' },
          { id: 'mesh-expert', name: 'mesh-expert', tool: 'agy', model: 'gemini-3.7-flash-high', status: 'active' },
        ],
        mode: 'runtime',
      },
    })

    const connections = screen.getAllByTestId('mesh-connection')
    expect(connections).toHaveLength(5)

    const centerConnection = connections[2]
    const commands = pathCommands(centerConnection)
    const controls = connectionControlXs(centerConnection)

    expect(commands).toEqual(['M', 'C'])
    expect(controls.control1).not.toBeCloseTo(controls.start, 3)
    expect(controls.control2).not.toBeCloseTo(controls.end, 3)
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

  it('shows a runtime role summary card after delayed hover', async () => {
    vi.useFakeTimers()

    render(MeshCanvas, {
      props: {
        lead,
        agents: [
          {
            ...makeAgents(1)[0],
            id: 'agent-1',
            name: 'frontend-dev',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
          },
        ],
        mode: 'runtime',
      },
    })

    const node = screen.getByTestId('mesh-node-agent')
    await fireEvent.mouseEnter(node)

    await vi.advanceTimersByTimeAsync(199)
    expect(screen.queryByTestId('mesh-node-role-card')).not.toBeInTheDocument()

    await vi.advanceTimersByTimeAsync(1)
    expect(screen.getByTestId('mesh-node-role-card')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-role-card-role-name')).toHaveTextContent('Codex Architect')
    expect(screen.getByTestId('mesh-node-role-card-tool-model')).toHaveTextContent('Codex · gpt-5')
    expect(screen.getByTestId('mesh-node-role-card-status')).toHaveTextContent('Active')
    expect(screen.getByTestId('mesh-node-role-card-summary')).toHaveTextContent(
      'Architecture decisions and structural review'
    )
    expect(screen.getByTestId('mesh-node-role-card-hint')).toHaveTextContent('Click for details')

    await fireEvent.mouseLeave(node)
    expect(screen.queryByTestId('mesh-node-role-card')).not.toBeInTheDocument()

    vi.useRealTimers()
  })

  it('shows the runtime role summary card on keyboard focus', async () => {
    vi.useFakeTimers()

    render(MeshCanvas, {
      props: {
        lead,
        agents: [
          {
            ...makeAgents(1)[0],
            id: 'agent-1',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
          },
        ],
        mode: 'runtime',
      },
    })

    const node = screen.getByTestId('mesh-node-agent')
    await fireEvent.focus(node)
    await vi.advanceTimersByTimeAsync(200)

    expect(screen.getByTestId('mesh-node-role-card')).toBeInTheDocument()

    await fireEvent.blur(node)
    expect(screen.queryByTestId('mesh-node-role-card')).not.toBeInTheDocument()

    vi.useRealTimers()
  })

  it('shows the runtime hover card placeholder even when role fields are empty', async () => {
    vi.useFakeTimers()

    render(MeshCanvas, {
      props: {
        lead,
        agents: [
          {
            ...makeAgents(1)[0],
            id: 'agent-1',
            name: 'developer1',
            roleName: '',
            focusArea: '',
            contextSummary: null,
            behaviorSummary: null,
            tool: 'codex',
            model: 'gpt-5.4 high',
          },
        ],
        mode: 'runtime',
      },
    })

    const node = screen.getByTestId('mesh-node-agent')
    await fireEvent.mouseEnter(node)
    await vi.advanceTimersByTimeAsync(200)

    expect(screen.getByTestId('mesh-node-role-card')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-role-card-name')).toHaveTextContent('developer1')
    expect(screen.getByTestId('mesh-node-role-card-tool-model')).toHaveTextContent('Codex · gpt-5.4 high')
    expect(screen.getByTestId('mesh-node-role-card-status')).toHaveTextContent('Active')
    expect(screen.getByTestId('mesh-node-role-card-placeholder-title')).toHaveTextContent('No role defined')
    expect(screen.getByTestId('mesh-node-role-card-placeholder-message')).toHaveTextContent(
      'Assign a role template to show a compact focus summary here.'
    )
    expect(screen.getByTestId('mesh-node-role-card-hint')).toHaveTextContent('Click for details')

    vi.useRealTimers()
  })

  it('suppresses the hover card while the click detail panel is open', async () => {
    vi.useFakeTimers()

    render(MeshCanvas, {
      props: {
        lead,
        agents: [
          {
            ...makeAgents(1)[0],
            id: 'agent-1',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
          },
        ],
        mode: 'runtime',
        selectedNodeId: 'agent-1',
      },
    })

    const node = screen.getByTestId('mesh-node-agent')
    await fireEvent.mouseEnter(node)
    await vi.advanceTimersByTimeAsync(250)

    expect(screen.queryByTestId('mesh-node-role-card')).not.toBeInTheDocument()

    vi.useRealTimers()
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
