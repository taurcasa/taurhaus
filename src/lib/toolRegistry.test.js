import { beforeEach, describe, expect, it } from 'vitest'
import {
  FALLBACK_TOOLS,
  configureToolRegistry,
  resetToolRegistry,
  toolDescriptor,
  tools,
} from './toolRegistry.js'

const CONTRACT_TOOLS = [
  {
    id: 'claude',
    label: 'Claude',
    accent: 'emerald',
    aliases: ['claude', 'claude_native'],
    capabilities: {
      modelFlag: '--model',
      effortFlag: { kind: 'argument', flag: '--effort' },
      displayNameFlag: '-n',
      teamFlags: true,
      nativeInboxPoller: true,
      authoritativeIdle: true,
      compactionHook: true,
      transcriptParser: true,
      catalog: true,
      configDirEnv: 'CLAUDE_CONFIG_DIR',
      usageBridge: true,
      notifySink: false,
      hookTrust: false,
    },
  },
  {
    id: 'codex',
    label: 'Codex',
    accent: 'sky',
    aliases: ['codex', 'mesh', 'mesh_bridged'],
    capabilities: {
      modelFlag: '-m',
      effortFlag: { kind: 'config', flag: '-c', key: 'model_reasoning_effort' },
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      authoritativeIdle: true,
      compactionHook: true,
      transcriptParser: true,
      catalog: true,
      configDirEnv: null,
      usageBridge: false,
      notifySink: true,
      hookTrust: true,
    },
  },
  {
    id: 'gemini',
    label: 'Gemini',
    accent: 'violet',
    aliases: ['gemini'],
    capabilities: {
      modelFlag: '-m',
      effortFlag: null,
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      authoritativeIdle: false,
      compactionHook: false,
      transcriptParser: false,
      catalog: true,
      configDirEnv: null,
      usageBridge: false,
      notifySink: false,
      hookTrust: false,
    },
  },
]

beforeEach(() => resetToolRegistry())

describe('toolRegistry', () => {
  it('keeps the pre-settings fallback byte-equivalent to the backend contract', () => {
    // Regression: commit cb32d7a added frontend tool branches independently of
    // backend detection; the fallback must be the same registry data as IPC.
    expect(FALLBACK_TOOLS).toEqual(CONTRACT_TOOLS)
  })

  it('uses descriptors supplied by the terminal platform contract', () => {
    const contract = structuredClone(CONTRACT_TOOLS)
    contract[1].label = 'Codex Contract'
    configureToolRegistry(contract)

    expect(tools()).toEqual(contract)
    expect(toolDescriptor('mesh').label).toBe('Codex Contract')
  })
})
