import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const realConsole = {
  log: console.log,
  warn: console.warn,
  error: console.error,
  debug: console.debug,
}

function restoreConsole() {
  console.log = realConsole.log
  console.warn = realConsole.warn
  console.error = realConsole.error
  console.debug = realConsole.debug
}

async function setupLogger() {
  vi.resetModules()
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-03-05T00:00:00.000Z'))

  const sink = {
    log: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }
  console.log = sink.log
  console.warn = sink.warn
  console.error = sink.error
  console.debug = sink.debug

  const tauriCore = await import('@tauri-apps/api/core')
  tauriCore.invoke.mockResolvedValue(undefined)
  await import('./logger.js')

  tauriCore.invoke.mockClear()
  sink.log.mockClear()
  sink.warn.mockClear()
  sink.error.mockClear()
  sink.debug.mockClear()

  return {
    invoke: tauriCore.invoke,
    sink,
  }
}

describe('logger bridge', () => {
  afterEach(() => {
    vi.useRealTimers()
    restoreConsole()
  })

  it('drops info/debug logs above the per-window rate limit and recovers in the next window', async () => {
    const { invoke } = await setupLogger()
    vi.setSystemTime(new Date('2026-03-05T00:00:01.001Z'))

    for (let i = 0; i < 30; i++) {
      console.log(`rate ${i}`)
    }

    expect(invoke).toHaveBeenCalledTimes(25)
    expect(invoke).toHaveBeenNthCalledWith(25, 'frontend_log', {
      level: 'info',
      message: 'rate 24',
    })

    vi.setSystemTime(new Date('2026-03-05T00:00:02.002Z'))
    console.log('rate after reset')

    expect(invoke).toHaveBeenCalledTimes(26)
    expect(invoke).toHaveBeenNthCalledWith(26, 'frontend_log', {
      level: 'info',
      message: 'rate after reset',
    })
  })

  it('skips configured noisy prefixes while forwarding other logs', async () => {
    const { invoke } = await setupLogger()

    console.log('[filewatch] changed')
    console.debug('[file] open: /tmp/file.txt')
    console.log('[code] highlighted block')
    console.debug('debug stays forwarded')

    expect(invoke).toHaveBeenCalledTimes(1)
    expect(invoke).toHaveBeenCalledWith('frontend_log', {
      level: 'debug',
      message: 'debug stays forwarded',
    })
  })

  it('handles IPC rejection without crashing and emits a fallback warning', async () => {
    const { invoke, sink } = await setupLogger()
    const error = new Error('ipc unavailable')

    invoke.mockRejectedValueOnce(error).mockResolvedValue(undefined)

    expect(() => console.log('trigger ipc failure')).not.toThrow()
    await Promise.resolve()
    await Promise.resolve()

    expect(sink.warn).toHaveBeenCalledWith(
      '[logger] failed to forward frontend log to backend:',
      error
    )
    expect(invoke).toHaveBeenNthCalledWith(1, 'frontend_log', {
      level: 'info',
      message: 'trigger ipc failure',
    })
    expect(invoke).toHaveBeenCalledTimes(2)
    const fallbackPayload = invoke.mock.calls[1][1]
    expect(fallbackPayload.level).toBe('warn')
    expect(fallbackPayload.message).toContain(
      '[logger] failed to forward frontend log to backend:'
    )
  })

  it('serializes edge-case values without throwing', async () => {
    const { invoke } = await setupLogger()
    const circular = {}
    circular.self = circular
    const largeObject = { blob: 'x'.repeat(2048) }

    expect(() =>
      console.log('serialize edge', circular, largeObject, 42n, Symbol('t'))
    ).not.toThrow()

    expect(invoke).toHaveBeenCalledTimes(1)
    const payload = invoke.mock.calls[0][1]
    expect(payload.level).toBe('info')
    expect(payload.message).toContain('serialize edge')
    expect(payload.message).toContain('[unserializable]')
    expect(payload.message).toContain('"blob":"')
    expect(payload.message).toContain('42')
    expect(payload.message).toContain('Symbol(t)')
  })
})
