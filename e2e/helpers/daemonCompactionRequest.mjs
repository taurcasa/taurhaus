/**
 * One NDJSON request to the taurhaus daemon, as its own process.
 *
 * `daemonCompaction.js` runs this with `execFileSync` because its caller is a
 * process-exit handler: there is no event loop left to await a socket on, and
 * the only synchronous socket Node offers is one in another process. Everything
 * arrives by environment; the matching response line, or nothing, goes to
 * stdout.
 */
import net from 'node:net'

const port = Number(process.env.TAURHAUS_DAEMON_PORT)
const host = process.env.TAURHAUS_DAEMON_HOST || '127.0.0.1'
const payload = process.env.TAURHAUS_DAEMON_REQUEST || ''
const timeoutMs = Number(process.env.TAURHAUS_DAEMON_TIMEOUT_MS || 4_000)
const requestId = JSON.parse(payload).id

function fail(message) {
  process.stderr.write(String(message))
  process.exit(1)
}

const socket = net.connect(port, host)
socket.setTimeout(timeoutMs)
socket.on('connect', () => socket.write(`${payload}\n`))
socket.on('timeout', () => {
  socket.destroy()
  fail(`no response within ${timeoutMs}ms`)
})
socket.on('error', (error) => fail(error?.message ?? String(error)))
socket.on('close', () => fail('daemon closed the connection without answering'))

let buffer = ''
socket.on('data', (chunk) => {
  buffer += chunk
  let newline = buffer.indexOf('\n')
  while (newline >= 0) {
    const line = buffer.slice(0, newline)
    buffer = buffer.slice(newline + 1)
    newline = buffer.indexOf('\n')
    // The daemon pushes events on the same stream; only the reply to this
    // request carries our id.
    let message
    try {
      message = JSON.parse(line)
    } catch {
      continue
    }
    if (message?.id !== requestId) continue
    socket.destroy()
    process.stdout.write(line)
    process.exit(0)
  }
})
