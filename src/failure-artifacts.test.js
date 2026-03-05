// @vitest-environment node
import { describe, it, expect, vi } from 'vitest'
import { appendDriverStderr, collectFailureArtifacts } from '../e2e/failure-artifacts.js'

describe('e2e failure artifact collection', () => {
  it('creates a per-spec artifact bundle with log tails, metadata, and screenshot', async () => {
    const writes = new Map()
    const createdDirs = []
    const screenshotPaths = []
    const jsonlLines = Array.from({ length: 205 }, (_, index) =>
      JSON.stringify({ event: `line-${index + 1}`, run_id: index === 204 ? 'run-123' : 'old-run' })
    ).join('\n')

    const deps = {
      mkdirSync: (path) => createdDirs.push(path),
      writeFileSync: (path, content) => writes.set(path, String(content)),
      existsSync: (path) =>
        path === '/logs/taurhaus.log.jsonl' || path === '/logs/daemon.log',
      readFileSync: (path) => {
        if (path === '/logs/taurhaus.log.jsonl') return jsonlLines
        if (path === '/logs/daemon.log') return 'daemon-line-1\ndaemon-line-2'
        return ''
      },
    }

    const bundle = await collectFailureArtifacts(
      {
        outputDir: '/tmp/wdio-output',
        specFile: '/repo/e2e/specs/git-workflow.js',
        testTitle: 'Git Workflow :: fails in diff view',
        groupIndex: 3,
        timestamp: '2026-03-06T00-00-00-000Z',
        appLogPaths: ['/logs/missing.log', '/logs/taurhaus.log.jsonl'],
        daemonLogPaths: ['/logs/daemon.log'],
        driverStderr: 'driver stderr output',
        saveScreenshot: async (path) => screenshotPaths.push(path),
      },
      deps
    )

    expect(bundle.artifactDir).toBe('/tmp/wdio-output/git-workflow/2026-03-06T00-00-00-000Z')
    expect(createdDirs).toContain('/tmp/wdio-output/git-workflow/2026-03-06T00-00-00-000Z')
    expect(screenshotPaths).toEqual([
      '/tmp/wdio-output/git-workflow/2026-03-06T00-00-00-000Z/failure.png',
    ])

    const metadataRaw = writes.get('/tmp/wdio-output/git-workflow/2026-03-06T00-00-00-000Z/metadata.json')
    expect(metadataRaw).toBeTruthy()
    const metadata = JSON.parse(metadataRaw)
    expect(metadata.spec_name).toBe('git-workflow')
    expect(metadata.group_index).toBe(3)
    expect(metadata.run_id).toBe('run-123')
    expect(metadata.app_log_source).toBe('/logs/taurhaus.log.jsonl')
    expect(metadata.daemon_log_source).toBe('/logs/daemon.log')
    expect(metadata.files).toEqual(
      expect.arrayContaining([
        'app-log.tail.log',
        'daemon-log.tail.log',
        'driver-stderr.log',
        'failure.png',
      ])
    )

    const appTail = writes.get('/tmp/wdio-output/git-workflow/2026-03-06T00-00-00-000Z/app-log.tail.log')
    expect(appTail).toContain('"line-205"')
    expect(appTail).not.toContain('"line-1"')
  })

  it('caps driver stderr buffer growth', () => {
    const capped = appendDriverStderr('a'.repeat(10), 'b'.repeat(10), 12)
    expect(capped).toBe('aabbbbbbbbbb')
  })
})
