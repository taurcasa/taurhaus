import { describe, expect, it } from 'vitest'

import {
  MODEL_OPTIONS_BY_TOOL,
  TOOL_OPTIONS,
  applyNamePattern,
  defaultModelForTool,
  modelsForTool,
  normalizeTool,
  resolveDefaultNamePattern,
  resolveRoleModel,
  resolveRoleTool,
  resolveSlotNamePattern,
  uniquifyMemberName,
} from './meshDefaults.js'

describe('meshDefaults', () => {
  it('exposes canonical tool/model defaults', () => {
    expect(TOOL_OPTIONS).toEqual(['claude', 'codex', 'gemini'])
    expect(MODEL_OPTIONS_BY_TOOL.claude).toEqual(['opus', 'sonnet', 'haiku'])
    expect(MODEL_OPTIONS_BY_TOOL.codex).toEqual(['gpt-5.4-high', 'gpt-5.3-codex', 'gpt-5-mini'])
    expect(MODEL_OPTIONS_BY_TOOL.gemini).toEqual(['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'])
  })

  it('normalizes tool values and provides model fallbacks', () => {
    expect(normalizeTool('CoDeX')).toBe('codex')
    expect(normalizeTool('')).toBe('claude')
    expect(normalizeTool('unknown')).toBe('claude')

    expect(modelsForTool('gemini')).toEqual(['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'])
    expect(modelsForTool('wat')).toEqual(['opus', 'sonnet', 'haiku'])
    expect(defaultModelForTool('codex')).toBe('gpt-5.4-high')
    expect(defaultModelForTool('wat')).toBe('opus')
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
    expect(resolveRoleModel({ model: 'gemini-2.5-pro' }, 'gemini')).toBe('gemini-2.5-pro')
    expect(resolveRoleModel({}, 'codex')).toBe('gpt-5.4-high')
  })

  it('applies deterministic member naming', () => {
    expect(applyNamePattern('agent-{project}-{n}', 2, 'taurhaus')).toBe('agent-taurhaus-2')

    const seen = new Map()
    expect(uniquifyMemberName('developer', seen)).toBe('developer')
    expect(uniquifyMemberName('developer', seen)).toBe('developer-1')
    expect(uniquifyMemberName('developer', seen)).toBe('developer-2')
    expect(uniquifyMemberName('', seen)).toBe('')
  })
})
