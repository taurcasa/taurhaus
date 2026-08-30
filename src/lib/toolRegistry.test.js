import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { beforeEach, describe, expect, it } from 'vitest'
import {
  FALLBACK_TOOLS,
  normalizeToolDescriptors,
  configureToolRegistry,
  resetToolRegistry,
  toolDescriptor,
  toolDisplayName,
  toolMedallionAccent,
  tools,
} from './toolRegistry.js'

const CONTRACT_TOOLS = JSON.parse(
  readFileSync(resolve(process.cwd(), 'src/lib/fixtures/tool-registry.json'), 'utf8')
)

beforeEach(() => resetToolRegistry())

describe('toolRegistry', () => {
  it('keeps the pre-settings fallback byte-equivalent to the backend contract', () => {
    // Regression: 07fc8f3 added frontend tool data independently of the Rust
    // registry; the shared fixture is also asserted by the Rust conformance test.
    expect(FALLBACK_TOOLS).toEqual(CONTRACT_TOOLS)
  })

  it('uses descriptors supplied by the terminal platform contract', () => {
    const contract = structuredClone(CONTRACT_TOOLS)
    contract[1].label = 'Codex Contract'
    configureToolRegistry(contract)

    expect(tools()).toEqual(contract)
    expect(toolDescriptor('mesh').label).toBe('Codex Contract')
  })

  it('preserves product names and runtime medallion accents from before the registry refactor', () => {
    // Regression: 91f4d3f replaced Settings product names and the runtime role
    // palette with the generic registry label/accent pair.
    expect(toolDisplayName('claude')).toBe('Claude Code')
    expect(toolDisplayName('agy')).toBe('Antigravity CLI')
    expect(toolMedallionAccent('claude')).toBe('amber')
    expect(toolMedallionAccent('codex')).toBe('emerald')
    expect(toolMedallionAccent('agy')).toBe('google-blue')
  })

  it('exposes Grok with its own accent and usage note', () => {
    // Regression: commit bfecae9 fixed the frontend registry at three
    // harnesses, so a fourth tool had no label, accent or usage explanation.
    expect(toolDisplayName('grok')).toBe('Grok CLI')
    expect(toolMedallionAccent('grok')).toBe('graphite')
    expect(toolDescriptor('grok').capabilities.autoApproveFlag).toBe('--always-approve')
    expect(toolDescriptor('grok').capabilities.usage).toBe(false)
    expect(toolDescriptor('grok').capabilities.usageNote).toBe(
      'Grok shows credits in its own /usage'
    )
  })

  it('exposes Antigravity without retaining Gemini as a tool alias', () => {
    // Regression: 9a66d1c hard-coded Gemini as the third frontend harness.
    expect(toolDisplayName('antigravity')).toBe('Antigravity CLI')
    expect(toolDescriptor('gemini')).toBeNull()
  })
})

// Regression: normalizeCapabilities/normalizeDescriptor spread their raw input
// but never dropped the snake_case aliases they consumed, so a snake payload
// produced two spellings of every capability (Opus review, 2026-08-30).
describe('toolRegistry drops the aliases it consumed', () => {
  it('leaves one spelling per capability and descriptor field', () => {
    const [tool] = normalizeToolDescriptors([
      {
        id: 'claude',
        label: 'Claude',
        display_name: 'Claude Code',
        medallion_accent: 'brand',
        default_agent_role_id: 'lead',
        capabilities: { model_flag: '--model', account_selector: 'CLAUDE_CONFIG_DIR', usage_note: 'n/a' },
      },
    ])
    expect(tool.displayName).toBe('Claude Code')
    expect(tool.capabilities.modelFlag).toBe('--model')
    expect(tool.capabilities.accountSelector).toBe('CLAUDE_CONFIG_DIR')
    for (const key of ['display_name', 'medallion_accent', 'default_agent_role_id']) {
      expect(tool).not.toHaveProperty(key)
    }
    for (const key of ['model_flag', 'account_selector', 'usage_note']) {
      expect(tool.capabilities).not.toHaveProperty(key)
    }
  })
})
