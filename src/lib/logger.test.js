import { afterEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}))

const realConsole = {
  log: console.log,
  info: console.info,
  warn: console.warn,
  error: console.error,
  debug: console.debug,
}

function restoreConsole() {
  console.log = realConsole.log
  console.info = realConsole.info
  console.warn = realConsole.warn
  console.error = realConsole.error
  console.debug = realConsole.debug
}

function payloadForCall(invoke, index) {
  return invoke.mock.calls[index][1].payload
}

async function flushLoggerBridge() {
  await vi.dynamicImportSettled()
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
  }
}

async function setupLogger() {
  vi.resetModules()
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-03-05T00:00:00.000Z'))
  window.__TAURI_INTERNALS__ = {}
  globalThis.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__

  const sink = {
    log: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }
  console.log = sink.log
  console.info = sink.info
  console.warn = sink.warn
  console.error = sink.error
  console.debug = sink.debug

  const tauriCore = await import('@tauri-apps/api/core')
  invokeMock.mockResolvedValue(undefined)
  await import('./logger.js')
  await flushLoggerBridge()

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
    delete window.__TAURI_INTERNALS__
    delete globalThis.__TAURI_INTERNALS__
    restoreConsole()
  })

  it('drops info/debug logs above the per-window rate limit and recovers in the next window', async () => {
    const { invoke } = await setupLogger()
    vi.setSystemTime(new Date('2026-03-05T00:00:01.001Z'))

    for (let i = 0; i < 30; i++) {
      console.log(`rate ${i}`)
    }
    await flushLoggerBridge()

    expect(invoke).toHaveBeenCalledTimes(25)
    expect(payloadForCall(invoke, 24)).toMatchObject({
      level: 'info',
      component: 'frontend',
      subsystem: 'console',
      event: 'frontend.console.received',
      message: 'rate 24',
    })

    vi.setSystemTime(new Date('2026-03-05T00:00:02.002Z'))
    console.log('rate after reset')
    await flushLoggerBridge()

    expect(invoke).toHaveBeenCalledTimes(26)
    expect(payloadForCall(invoke, 25)).toMatchObject({
      level: 'info',
      message: 'rate after reset',
    })
  })

  it('forwards console.info as info level', async () => {
    const { invoke } = await setupLogger()

    console.info('info channel works', { source: 'test' })
    await flushLoggerBridge()

    expect(invoke).toHaveBeenCalledTimes(1)
    expect(payloadForCall(invoke, 0)).toMatchObject({
      level: 'info',
      message: 'info channel works {"source":"test"}',
      context: { source: 'test' },
    })
  })

  it('skips configured noisy prefixes while forwarding other logs', async () => {
    const { invoke } = await setupLogger()

    console.log('[filewatch] changed')
    console.debug('[file] open: /tmp/file.txt')
    console.log('[code] highlighted block')
    console.debug('debug stays forwarded')
    await flushLoggerBridge()

    expect(invoke).toHaveBeenCalledTimes(1)
    expect(payloadForCall(invoke, 0)).toMatchObject({
      level: 'debug',
      component: 'frontend',
      subsystem: 'console',
      event: 'frontend.console.received',
      message: 'debug stays forwarded',
    })
  })

  it('handles IPC rejection without crashing and does not recurse forwarding', async () => {
    const { invoke, sink } = await setupLogger()
    const error = new Error('ipc unavailable')

    invoke.mockRejectedValue(error)

    expect(() => console.log('trigger ipc failure')).not.toThrow()
    await flushLoggerBridge()

    expect(sink.warn).toHaveBeenCalledWith(
      '[logger] failed to forward frontend log to backend:',
      error
    )
    expect(payloadForCall(invoke, 0)).toMatchObject({
      level: 'info',
      message: 'trigger ipc failure',
    })
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it('adds interaction_id for logs in the same user interaction chain', async () => {
    const { invoke } = await setupLogger()
    vi.setSystemTime(new Date('2026-03-05T00:00:01.500Z'))

    window.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    console.log('first in click')
    console.debug('second in click')
    await flushLoggerBridge()

    const firstInteractionId = payloadForCall(invoke, 0).interaction_id
    const secondInteractionId = payloadForCall(invoke, 1).interaction_id
    expect(firstInteractionId).toBeTypeOf('string')
    expect(secondInteractionId).toBe(firstInteractionId)

    vi.setSystemTime(new Date('2026-03-05T00:00:05.000Z'))
    console.log('outside interaction window')
    await flushLoggerBridge()
    expect(payloadForCall(invoke, 2).interaction_id).toBeUndefined()

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    console.log('new interaction')
    await flushLoggerBridge()
    expect(payloadForCall(invoke, 3).interaction_id).toBeTypeOf('string')
    expect(payloadForCall(invoke, 3).interaction_id).not.toBe(firstInteractionId)
  })

  it('emits frontend.logs.dropped with dropped_count when logs are throttled', async () => {
    const { invoke } = await setupLogger()
    vi.setSystemTime(new Date('2026-03-05T00:00:01.000Z'))

    for (let i = 0; i < 30; i++) {
      console.log(`drop ${i}`)
    }
    await flushLoggerBridge()
    expect(invoke).toHaveBeenCalledTimes(25)

    vi.setSystemTime(new Date('2026-03-05T00:00:06.500Z'))
    console.log('flush dropped metrics')
    await flushLoggerBridge()

    const payloads = invoke.mock.calls.map(call => call[1].payload)
    const droppedEvent = payloads.find(payload => payload.event === 'frontend.logs.dropped')
    expect(droppedEvent).toBeTruthy()
    expect(droppedEvent).toMatchObject({
      level: 'warn',
      component: 'frontend',
      subsystem: 'logger',
      dropped_count: 5,
      dropped_reason_counts: { rate_limit: 5 },
    })
  })

  it('serializes edge-case values without throwing', async () => {
    const { invoke } = await setupLogger()
    const circular = {}
    circular.self = circular
    const largeObject = { blob: 'x'.repeat(2048) }

    expect(() =>
      console.log('serialize edge', circular, largeObject, 42n, Symbol('t'))
    ).not.toThrow()
    await flushLoggerBridge()

    expect(invoke).toHaveBeenCalledTimes(1)
    const payload = payloadForCall(invoke, 0)
    expect(payload.level).toBe('info')
    expect(payload.message).toContain('serialize edge')
    expect(payload.message).toContain('[unserializable]')
    expect(payload.message).toContain('"blob":"')
    expect(payload.message).toContain('42')
    expect(payload.message).toContain('Symbol(t)')
    expect(payload.context).toMatchObject({
      blob: largeObject.blob,
    })
  })
})
