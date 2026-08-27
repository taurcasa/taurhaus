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
 * A PNG whose header says what size it is — the part of the file the lane
 * reads. Everything after IHDR is irrelevant to the check, so the pixels are
 * not there.
 */
function pngFile(path, width, height) {
  const png = Buffer.alloc(33)
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(png, 0)
  png.writeUInt32BE(13, 8)
  png.write('IHDR', 12, 'ascii')
  png.writeUInt32BE(width, 16)
  png.writeUInt32BE(height, 20)
  png[24] = 8 // bit depth
  png[25] = 6 // truecolour with alpha
  writeFileSync(path, png)
  return path
}

/**
 * A stand-in for Windows Edge: it prints a DOM and writes a PNG, or fails, or
 * hangs — with or without listening to the signal that asks it to stop. All of
 * it is what the real one can do to this script.
 */
function fakeEdge(dir, {
  mode = 'ok',
  fixture = 'shell-popups/chooser-light/laptop/light',
  width = 1366,
  height = 768,
} = {}) {
  const path = join(dir, 'fake-edge.sh')
  const shot = mode === 'notpng'
    ? (writeFileSync(join(dir, 'fake-shot.png'), 'png'), join(dir, 'fake-shot.png'))
    : pngFile(join(dir, 'fake-shot.png'), width, height)
  writeFileSync(path, `#!/usr/bin/env bash
set -u
# A hung renderer does not stop for TERM, and a lane that only asks waits.
if [[ "${mode}" == "deaf" ]]; then trap '' TERM; for _ in $(seq 1 60); do sleep 0.5; done; fi
if [[ "${mode}" == "slow" ]]; then sleep 5; fi
if [[ "${mode}" == "fail" ]]; then echo "edge crashed" >&2; exit 3; fi
for arg in "$@"; do
  if [[ "$arg" == --screenshot=* ]]; then
    win="\${arg#--screenshot=}"
    cp "${shot}" "${dir}/\${win##*\\\\}"
  fi
done
echo '<html><body><main data-visual-host-root data-visual-host-fixture="${fixture}" data-visual-host-request="ok"></main></body></html>'
`)
  chmodSync(path, 0o755)
  return path
}

const SHOT_ARGS = ['shell-popups', 'chooser-light', 'laptop', 'light']

function runShot(dir, port, edge, extraEnv = {}, args = SHOT_ARGS) {
  return new Promise((resolve) => {
    const child = spawn('bash', [SCRIPT, ...args], {
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

    const result = await runShot(
      dir,
      port,
      fakeEdge(dir, { fixture: 'mesh-canvas/idle/laptop/light' })
    )

    expect(result.code).toBe(7)
    expect(result.stderr).toContain('mesh-canvas/idle')
  })

  // Regression: 6ec843e checked the component and the scenario the page
  // rendered, and nothing else. The theme and the viewport are half of what a
  // popup screenshot is evidence about.
  it('refuses a page that rendered another theme or viewport', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const themed = await runShot(
      dir,
      port,
      fakeEdge(dir, { fixture: 'shell-popups/chooser-light/laptop/dark' })
    )
    expect(themed.code).toBe(7)
    expect(themed.stderr).toContain('shell-popups/chooser-light/laptop/dark')

    const sized = await runShot(
      dir,
      port,
      fakeEdge(dir, { fixture: 'shell-popups/chooser-light/narrow/light' })
    )
    expect(sized.code).toBe(7)
    expect(sized.stderr).toContain('shell-popups/chooser-light/narrow/light')
  })

  // Regression: 6ec843e matched the fixture the page reported with a plain
  // `grep`, so the requested address was read as a regular expression. A
  // component or scenario carrying metacharacters matched whatever the host had
  // fallen back to, and the fallback was filed under the requested name.
  it('refuses a fixture address that matches only as a regular expression', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(
      dir,
      port,
      fakeEdge(dir, { fixture: 'mesh-canvas/idle/laptop/light' }),
      {},
      ['.*', '.*', 'laptop', 'light']
    )

    expect(result.code).toBe(7)
    expect(result.stderr).toContain('mesh-canvas/idle/laptop/light')
  })

  // Regression: 6ec843e checked that a file arrived, not that it was the shot
  // that was asked for. A browser rendering at another window size or device
  // scale wrote a PNG of the wrong pixel size, and the lane filed it as
  // evidence about a viewport it never showed.
  it('refuses a screenshot whose pixels are not the viewport preset', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir, { width: 683, height: 384 }))

    expect(result.code).toBe(10)
    expect(result.stderr).toContain('683x384')
    expect(result.stderr).toContain('1366x768')
  })

  // Regression: 6ec843e accepted any non-empty file, so a browser that wrote an
  // error page, a truncated file, or three bytes passed as a screenshot.
  it('refuses a screenshot that is not a PNG', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir, { mode: 'notpng' }))

    expect(result.code).toBe(10)
    expect(result.stderr).toContain('not a PNG')
  })

  // Regression: 74c7761 put whatever `theme` said straight into the URL. The
  // host falls back for a theme it does not know, so `theme=drak` shot the
  // scenario's own theme, filed it under `drak`, and exited 0.
  it('refuses a theme that is neither light nor dark', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const result = await runShot(dir, port, fakeEdge(dir), {}, [
      'shell-popups',
      'chooser-light',
      'laptop',
      'drak',
    ])

    expect(result.code).toBe(2)
    expect(result.stderr).toContain("Unknown theme 'drak'")
    expect(readdirSync(dir)).not.toContain('shell-popups-chooser-light-laptop-drak.png')
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

  // Regression: 6ec843e wrapped the browser in a plain `timeout`, which asks
  // with TERM and never insists. A renderer that ignores it — a hung one does —
  // held the lane for as long as it liked, which is what the wall clock was
  // added to prevent.
  it('kills a browser that ignores the timeout', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'visual-shot-'))
    const port = await startServer(HOST_PAGE)

    const started = Date.now()
    const result = await runShot(dir, port, fakeEdge(dir, { mode: 'deaf' }), {
      VISUAL_SHOT_TIMEOUT_S: '1',
      VISUAL_SHOT_KILL_AFTER_S: '1',
    })

    expect(result.code).toBe(9)
    expect(result.stderr).toContain('timed out')
    expect(Date.now() - started).toBeLessThan(15_000)
  }, 20_000)
})
