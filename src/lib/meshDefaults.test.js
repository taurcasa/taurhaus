import { describe, expect, it } from 'vitest'

import {
  toolOptions,
  applyNamePattern,
  normalizeTool,
  resolveDefaultNamePattern,
  resolveRoleModel,
  resolveRoleReasoningEffort,
  resolveRoleTool,
  resolveSlotNamePattern,
  uniquifyMemberName,
} from './meshDefaults.js'

describe('meshDefaults', () => {
  it('exposes the canonical tool list', () => {
    expect(toolOptions()).toEqual(['claude', 'codex', 'gemini'])
  })

  it('normalizes tool values', () => {
    expect(normalizeTool('CoDeX')).toBe('codex')
    expect(normalizeTool('')).toBe('claude')
    expect(normalizeTool('unknown')).toBe('claude')
  })

  it('resolves role defaults from camel/snake payload variants', () => {
    const role = {
      cli_tool: 'gemini',
      defaults: {
        default_name_pattern: 'ui-{project}-{n}',
      },
    }

    expect(resolveDefaultNamePattern(role)).toBe('ui-{project}-{n}')
    expect(resolveSlotNamePattern({}, role)).toBe('ui-{project}-{n}')
    expect(resolveSlotNamePattern({ overrides: { namePattern: 'specialist-{n}' } }, role)).toBe(
      'specialist-{n}'
    )

    expect(resolveRoleTool(role, 'codex')).toBe('gemini')
    expect(resolveRoleTool({}, 'codex')).toBe('codex')
    // Regression: two hardcoded model lists (meshDefaults.js, RoleEditor.svelte)
    // shadowed the backend catalog; a role now reports only what it declares and
    // the catalog supplies the default.
    expect(resolveRoleModel({ model: 'gemini-3.1-pro' })).toBe('gemini-3.1-pro')
    expect(resolveRoleModel({ model: '' })).toBe('')
    expect(resolveRoleModel({})).toBe('')
    expect(resolveRoleModel({ defaults: { model: 'gpt-5.6-terra' } })).toBe('gpt-5.6-terra')
    expect(resolveRoleReasoningEffort({ defaults: { reasoning_effort: 'high' } })).toBe('high')
    expect(resolveRoleReasoningEffort({ reasoningEffort: 'low' })).toBe('low')
    expect(resolveRoleReasoningEffort({})).toBeNull()
  })

  it('applies deterministic member naming', () => {
    expect(applyNamePattern('agent-{project}-{n}', 2, 'taurhaus')).toBe('agent-taurhaus-2')

    const seen = new Map()
    expect(uniquifyMemberName('developer', seen)).toBe('developer')
    expect(uniquifyMemberName('developer', seen)).toBe('developer-1')
    expect(uniquifyMemberName('developer', seen)).toBe('developer-2')
    expect(uniquifyMemberName('', seen)).toBe('')
  })

  it('continues numbered member names instead of appending nested suffixes', () => {
    const seen = new Map()
    expect(uniquifyMemberName('dev-1', seen)).toBe('dev-1')
    expect(uniquifyMemberName('dev-1', seen)).toBe('dev-2')
    expect(uniquifyMemberName('dev-1', seen)).toBe('dev-3')
  })
})
