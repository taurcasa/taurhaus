import { describe, expect, it } from 'vitest'

import { normalizeInvokeError } from './client.js'

describe('normalizeInvokeError', () => {
  // Regression: the lossless error normalizer used Object.assign(new Error(), payload),
  // so a backend payload carrying `stack` or `name` would replace the thrown
  // error's own — and `code`/`command`/`retryable` were written even when
  // invalid (Opus review of the lossless-normalizer change, 2026-08-30).
  it('keeps every backend field but never the Error-owned ones', () => {
    const error = normalizeInvokeError({
      code: 'BOOM',
      message: 'boom',
      command: 'do_thing',
      retryable: 'not-a-boolean',
      stack: 'FAKE STACK',
      name: 'FakeName',
      detail: 'kept',
    })
    expect(error).toBeInstanceOf(Error)
    expect(error.stack).not.toBe('FAKE STACK')
    expect(error.name).toBe('Error')
    expect(error.code).toBe('BOOM')
    expect(error.command).toBe('do_thing')
    expect(error.detail).toBe('kept')
    expect(error).not.toHaveProperty('retryable')
    expect(error.ipc).toEqual({ code: 'BOOM', command: 'do_thing', retryable: null })
  })
})
