/** Shared IPC transport with mock fallback. */
import { formatUserFacingError } from '../format.js'

export function isTauri() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function normalizeInvokeError(error) {
  if (error instanceof Error) return error

  let parsed = error
  if (typeof error === 'string') {
    try {
      parsed = JSON.parse(error)
    } catch {
      parsed = error
    }
  }

  if (parsed && typeof parsed === 'object') {
    const code = typeof parsed.code === 'string' && parsed.code.trim() ? parsed.code : null
    const command = typeof parsed.command === 'string' && parsed.command.trim() ? parsed.command : null
    const retryable = typeof parsed.retryable === 'boolean' ? parsed.retryable : null
    const message = formatUserFacingError(parsed, code ?? "Couldn't complete the request")
    const normalized = new Error(message)
    Object.assign(normalized, parsed)
    normalized.message = message
    if (code) normalized.code = code
    if (command) normalized.command = command
    if (retryable !== null) normalized.retryable = retryable
    normalized.ipc = {
      code,
      command,
      retryable,
    }
    return normalized
  }

  if (typeof parsed === 'string') {
    return new Error(parsed)
  }

  return new Error("Couldn't complete the request")
}

/**
 * Call Rust IPC command in Tauri, or a supplied mock implementation in web/dev mode.
 */
export async function invokeOrMock(command, args, mockFn) {
  if (!isTauri()) {
    return mockFn()
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    return args === undefined ? invoke(command) : invoke(command, args)
  } catch (error) {
    const normalized = normalizeInvokeError(error)
    const context = {
      command,
      has_args: args !== undefined,
      code: normalized.code ?? null,
      retryable: normalized.retryable ?? null,
      error_message: normalized.message,
    }

    if (normalized.retryable === true) {
      console.warn('[ipc] invoke failed with retryable error', context)
    } else {
      console.error('[ipc] invoke failed', context)
    }

    throw normalized
  }
}
