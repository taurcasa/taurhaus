import { describe, expect, it } from 'vitest'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const specSource = readFileSync(join(process.cwd(), 'e2e/specs/compaction-codex-hooks.js'), 'utf8')
const recipeSource = readFileSync(join(process.cwd(), 'scripts/test-compaction-codex.py'), 'utf8')
const analyzerSource = readFileSync(join(process.cwd(), 'scripts/analyze-compaction.py'), 'utf8')

describe('Codex compaction defaults', () => {
  it('keeps the paid lane on the default hook path without a retired mode override', () => {
    // Regression: commit 3b56a3f explicitly changed harness.codex_compaction
    // and the shared daemon even though hooks are now the only supported path.
    expect(specSource).toContain('expectRetiredCodexCompactionSettingAbsent')
    expect(specSource).not.toMatch(/set_codex_compaction_mode|setCodexCompactionMode|codexCompactionMode/)
    expect(specSource).not.toMatch(/codexCompaction\s*:|codex_compaction\s*:/)
  })

  it('drives the Codex recipe through automatic compaction and native hook events', () => {
    // Regression: commit 27770fbd made the recipe send manual /compact and
    // wait for transcript-owner events, which cannot exercise Codex hooks.
    expect(recipeSource).toContain('trigger_mode="automatic"')
    expect(recipeSource).toContain('compaction.codex_hook.delivered')
    expect(recipeSource).not.toContain('"/compact"')
    expect(recipeSource).not.toContain('compaction.detected')
  })

  it('matches Codex hook outcomes by session before member resolution', () => {
    // Regression: 076c3bf filtered Codex received and terminal hook events by
    // team/member even though unresolved events do not carry those fields,
    // causing successful runs to time out and skipped runs to burn all turns.
    expect(recipeSource.match(/team_name=None/g)).toHaveLength(2)
    expect(recipeSource.match(/member_name=None/g)).toHaveLength(2)
  })

  it('keeps the analyzer vocabulary limited to native hook events', () => {
    // Regression: commit 27770fbd taught the analyzer to treat extractor and
    // watcher events as the canonical compaction trail.
    expect(analyzerSource).toContain('HOOK_EVENT_PREFIXES')
    expect(analyzerSource).not.toMatch(/compaction\.(detected|signal_|extractor|watcher|owner)/)
  })

  it('reports only native hook records from an isolated log', () => {
    // Regression: 076c3bf gave the received fixture resolved member fields,
    // hiding that --team/--member discarded the hook's first checkpoint.
    const root = mkdtempSync(join(tmpdir(), 'taurhaus-hook-analyzer-'))
    const logPath = join(root, 'taurhaus.log.jsonl')
    try {
      writeFileSync(logPath, [
        '{"ts":"2026-08-28T10:00:00Z","event":"compaction.codex_hook.received","tool":"codex","session_id":"session-1"}',
        '{"ts":"2026-08-28T10:00:01Z","event":"compaction.injected","tool":"codex","team_name":"alpha","member_name":"builder"}',
        '{"ts":"2026-08-28T10:00:02Z","event":"compaction.codex_hook.delivered","tool":"codex","team_name":"alpha","member_name":"builder","session_id":"session-1","additional_context_bytes":42}',
      ].join('\n'))

      const output = execFileSync('python3', [
        join(process.cwd(), 'scripts/analyze-compaction.py'),
        '--log', logPath,
        '--team', 'alpha',
      ], { encoding: 'utf8' })

      expect(output).toContain('Hook events: 2')
      expect(output).toContain('received: 1')
      expect(output).toContain('delivered: 1')
      expect(output).not.toContain('compaction.injected')
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
