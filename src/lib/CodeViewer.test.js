import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

function createDeferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

vi.mock('./markdown.js', () => ({
  highlightCode: vi.fn(),
}))

const { highlightCode } = await import('./markdown.js')
import CodeViewer from './CodeViewer.svelte'

describe('CodeViewer', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    highlightCode.mockResolvedValue('<pre class="shiki"><code><span class="line">default</span></code></pre>')
  })

  it('ignores stale highlight results from older requests', async () => {
    const slow = createDeferred()
    const fast = createDeferred()

    highlightCode.mockImplementation((source) => {
      if (source === 'old()') return slow.promise
      if (source === 'new()') return fast.promise
      return Promise.resolve('<pre class="shiki"><code><span class="line">default</span></code></pre>')
    })

    const view = render(CodeViewer, {
      props: {
        code: 'old()',
        language: 'javascript',
      },
    })
    await view.rerender({
      code: 'new()',
      language: 'javascript',
    })

    fast.resolve('<pre class="shiki"><code><span class="line">new</span></code></pre>')
    await waitFor(() => {
      expect(view.container.querySelector('.code-highlighted')).toHaveTextContent('new')
    })

    slow.resolve('<pre class="shiki"><code><span class="line">old</span></code></pre>')
    await waitFor(() => {
      expect(view.container.querySelector('.code-highlighted')).toHaveTextContent('new')
    })
    expect(view.container.querySelector('.code-highlighted')).not.toHaveTextContent('old')
  })
})
