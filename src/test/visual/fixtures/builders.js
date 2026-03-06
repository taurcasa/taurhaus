function cycle(list, index) {
  return list[index % list.length]
}

export function createMember({
  id,
  name,
  role = 'agent',
  tool = 'claude',
  toolLabel = 'Claude',
  model = '',
  status = 'offline',
  position = { x: 0, y: 0 },
} = {}) {
  return {
    id,
    name,
    role,
    tool,
    toolLabel,
    model,
    status,
    position,
  }
}

export function createConnection(from, to) {
  return {
    id: `${from}-${to}`,
    from,
    to,
  }
}

export function createLeadMember(overrides = {}) {
  return createMember({
    id: 'lead-1',
    name: 'team-lead',
    role: 'lead',
    tool: 'claude',
    toolLabel: 'Claude',
    model: 'opus',
    status: 'active',
    position: { x: 450, y: 156 },
    ...overrides,
  })
}

export function createAgentMembers(count, {
  canvasSize = { width: 900, height: 520 },
  statusCycle = ['active', 'idle', 'active', 'idle', 'offline'],
  toolCycle = ['codex', 'gemini', 'claude'],
} = {}) {
  const width = Number(canvasSize.width ?? 900)
  const height = Number(canvasSize.height ?? 520)
  const leadY = Math.round(height * 0.3)
  const baseAgentY = Math.round(height * 0.65)
  const wrap = count >= 7
  const row1Count = wrap ? Math.ceil(count / 2) : count
  const row2Count = wrap ? count - row1Count : 0

  function rowPosition(rowIndex, indexInRow, rowCount) {
    const usableWidth = width - 120
    const gap = rowCount <= 1 ? 0 : usableWidth / (rowCount - 1)
    const startX = width / 2 - usableWidth / 2
    const rowOffset = wrap ? 44 : 0
    const y = rowIndex === 0 ? baseAgentY - rowOffset : baseAgentY + rowOffset
    return {
      x: Math.round(startX + gap * indexInRow),
      y,
    }
  }

  return Array.from({ length: count }, (_, index) => {
    const tool = cycle(toolCycle, index)
    const toolLabel = tool === 'codex' ? 'Codex' : tool === 'gemini' ? 'Gemini' : 'Claude'
    const rowIndex = wrap && index >= row1Count ? 1 : 0
    const rowOffset = rowIndex === 0 ? index : index - row1Count
    const rowCount = rowIndex === 0 ? row1Count : row2Count

    return createMember({
      id: `agent-${index + 1}`,
      name: `agent-${index + 1}`,
      role: 'agent',
      tool,
      toolLabel,
      model: tool === 'codex' ? 'gpt-5.4 high' : tool === 'gemini' ? '2.5-pro' : 'sonnet',
      status: cycle(statusCycle, index),
      position: rowPosition(rowIndex, rowOffset, rowCount),
    })
  })
}
