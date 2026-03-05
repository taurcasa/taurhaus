/** Shared IPC transport with mock fallback. */
export function isTauri() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function normalizeInvokeError(error) {
  if (error instanceof Error) return error

  if (error && typeof error === 'object') {
    const message =
      typeof error.message === 'string'
        ? error.message
        : typeof error.code === 'string'
          ? `${error.code}`
          : 'IPC request failed'
    return new Error(message)
  }

  if (typeof error === 'string') {
    return new Error(error)
  }

  return new Error('IPC request failed')
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
    throw normalizeInvokeError(error)
  }
}
