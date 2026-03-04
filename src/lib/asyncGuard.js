/**
 * Sequence-token guard for async calls.
 * Call next() before each request and only apply results when isCurrent(token) is true.
 */
export function createAsyncGuard() {
  let sequence = 0

  return {
    next() {
      sequence += 1
      return sequence
    },
    isCurrent(token) {
      return token === sequence
    },
    invalidate() {
      sequence += 1
      return sequence
    },
  }
}
