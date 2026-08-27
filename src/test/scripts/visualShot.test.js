/**
 * `scripts/visual-shot.sh` — the screenshot lane's own guardrails.
 *
 * The lane exists to produce evidence, so a shot that succeeds on the wrong
 * page is worse than one that fails: it is filed as proof. These tests drive
 * the real script against a fake server and a fake browser, and check that
 * every way of ending up with an irrelevant PNG ends as a failure instead.
 */

import { createServer } from 'node:http'
import { chmodSync, mkdtempSync, readdirSync, writeFileSync } from 'node:fs'
import { spawn } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

// Vitest runs from the checkout root, which is where the script lives.
const SCRIPT = resolve(process.cwd(), 'scripts/visual-shot.sh')
const HOST_PAGE = '<!doctype html><html><head><meta name="taurhaus-visual-host" content="1">'
  + '</head><body><div id="app"></div></body></html>'

let server = null

function startServer(body) {
  return new Promise((resolve) => {
    server = createServer((_request, response) => {
      response.writeHead(200, { 'content-type': 'text/html' })
      response.end(body)
    })
    server.listen(0, () => resolve(server.address().port))
  })
}

/**
 * A stand-in for Windows Edge: it prints a DOM and writes a PNG, or fails, or
 * hangs — the three things the real one can do to this script.
 */
function fakeEdge(dir, { mode = 'ok', fixture = 'shell-popups/chooser-light' } = {}) {
  const path = join(dir, 'fake-edge.sh')
  writeFileSync(path, `#!/usr/bin/env bash
set -u
if [[ "${mode}" == "slow" ]]; then sleep 5; fi
if [[ "${mode}" == "fail" ]]; then echo "edge crashed" >&2; exit 3; fi
for arg in "$@"; do
  if [[ "$arg" == --screenshot=* ]]; then
    win="\${arg#--screenshot=}"
    printf 'png' > "${dir}/\${win##*\\\\}"
  fi
done
echo '<html><body><main data-visual-host-root data-visual-host-fixture="${fixture}" data-visual-host-request="ok"></main></body></html>'
`)
  chmodSync(path, 0o755)
  return path
}

function runShot(dir, port, edge, extraEnv = {}) {
  return new Promise((resolve) => {
    const child = spawn('bash', [SCRIPT, 'shell-popups', 'chooser-light', 'laptop', 'light'], {
      env: {
        ...process.env,
        VISUAL_SHOT_PORT: String(port),
        VISUAL_SHOT_EDGE: edge,
        VISUAL_SHOT_WINDOWS_DIR: 'C:\\fake\\shots',
        VISUAL_SHOT_WSL_DIR: dir,
        VISUAL_SHOT_BUDGET_MS: '10',
        ...extraEnv,
      },
    })
    let stdout = ''
    let stderr = ''
    child.stdout.on('data', (chunk) => { stdout += chunk })
    child.stderr.on('data', (chunk) => { stderr += chunk })
    child.on('close', (code) => resolve({ code, stdout, stderr }))
  })
}

describe('visual-shot.sh', () => {
  afterEach(async () => {
    if (server) await new Promise((resolve) => server.close(resolve))
    server = null
  })

  it('shoots the fixture it was asked for', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir))

    expect(result.code).toBe(0)
    // The path is the script's answer, so it is the last thing it says.
    expect(result.stdout.trim().split('\n').at(-1)).toBe(
      `${dir}/shell-popups-chooser-light-laptop-light.png`
    )
    expect(readdirSync(dir)).toContain('shell-popups-chooser-light-laptop-light.png')
  })

  // Regression: 74c7761 reused any listener on the port, so a `bun run dev` or
  // an unrelated server produced a screenshot of somebody else's page.
  it('refuses a port held by something that is not the visual host', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer('<html><body>some other dev server</body></html>')

    const result = await runShot(dir, port, fakeEdge(dir))

    expect(result.code).toBe(6)
    expect(result.stderr).toContain('not the visual host')
  })

  // Regression: 74c7761 let an unknown component or scenario fall back to the
  // first fixture in the registry, so a mistyped shot was filed as evidence.
  it('refuses a page that rendered a different fixture', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir, { fixture: 'mesh-canvas/idle' }))

    expect(result.code).toBe(7)
    expect(result.stderr).toContain('mesh-canvas/idle')
  })

  // Regression: 74c7761 discarded Edge's exit status with `|| true`.
  it('reports a browser that failed', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir, { mode: 'fail' }))

    expect(result.code).toBe(8)
    expect(result.stderr).toContain('Edge failed')
  })

  // Regression: 74c7761 bounded the page's virtual time but not the process, so
  // a hung browser hung the lane.
  it('gives up on a browser that never exits', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir, { mode: 'slow' }), {
      VISUAL_SHOT_TIMEOUT_S: '1',
    })

    expect(result.code).toBe(9)
    expect(result.stderr).toContain('timed out')
  }, 20_000)
})
