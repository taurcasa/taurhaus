import { describe, expect, it } from 'vitest'

import {
  latestRoleVersions,
  parseRoleVersionNumber,
  roleVersionGroupKey,
  stripRoleVersionPrefix,
} from './roleVersioning.js'

describe('roleVersioning', () => {
  it('parses and strips version prefixes', () => {
    expect(parseRoleVersionNumber({ roleId: 'v3-codex-developer' })).toBe(3)
    expect(stripRoleVersionPrefix('v12-claude-reviewer')).toBe('claude-reviewer')
  })

  it('groups conceptual versions by tool, kind, and role function', () => {
    expect(
      roleVersionGroupKey({
        roleId: 'v2-codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
      })
    ).toBe('codex:agent:developer')
  })

  it('keeps the highest prefixed version for the same conceptual role', () => {
    const roles = [
      {
        roleId: 'v2-codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
      },
      {
        roleId: 'v3-codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
      },
    ]

    expect(latestRoleVersions(roles)).toEqual([roles[1]])
  })

  it('falls back to importedAt timestamps when no version prefix exists', () => {
    const roles = [
      {
        roleId: 'claude-reviewer-legacy',
        name: 'Claude Reviewer',
        kind: 'agent',
        cliTool: 'claude',
        provenance: { importedAt: '2026-03-09T08:00:00Z' },
      },
      {
        roleId: 'claude-reviewer-current',
        name: 'Claude Reviewer',
        kind: 'agent',
        cliTool: 'claude',
        provenance: { importedAt: '2026-03-11T08:00:00Z' },
      },
    ]

    expect(latestRoleVersions(roles)).toEqual([roles[1]])
  })
})
