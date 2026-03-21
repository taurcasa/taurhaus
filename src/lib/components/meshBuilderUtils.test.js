import { describe, expect, it } from 'vitest'

import { createAgentFromRole } from './meshBuilderUtils.js'

describe('meshBuilderUtils', () => {
  it('continues numbered names when adding another member for the same role', () => {
    const agent = createAgentFromRole(
      {
        roleId: 'codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        defaultNamePattern: 'dev-{n}',
      },
      '/projects/taurhaus',
      [
        { name: 'dev-1' },
        { name: 'dev-2' },
      ]
    )

    expect(agent.name).toBe('dev-3')
    expect(agent.id).toBe('dev-3')
  })
})
