import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./markdown.js', () => ({
  renderMarkdown: vi.fn(),
}))

vi.mock('./ipc.js', () => ({
  readProjectAsset: vi.fn(() => Promise.resolve(null)),
  openExternalUrl: vi.fn(() => Promise.resolve()),
}))

vi.mock('./assetCache.js', () => ({
  get: vi.fn(() => null),
  set: vi.fn(),
}))

vi.mock('./pathUtils.js', () => ({
  resolveRelativePath: vi.fn((base, rel) => rel),
}))

vi.mock('dompurify', () => ({
  default: {
    sanitize: vi.fn((html) => html),
  },
}))

const { mockMermaidRender, mockMermaidInitialize } = vi.hoisted(() => ({
  mockMermaidRender: vi.fn(),
  mockMermaidInitialize: vi.fn(),
}))

vi.mock('mermaid', () => ({
  default: {
    initialize: mockMermaidInitialize,
    render: mockMermaidRender,
  },
}))

const { renderMarkdown } = await import('./markdown.js')
const { default: DOMPurify } = await import('dompurify')
const { openExternalUrl } = await import('./ipc.js')
import MarkdownRenderer from './MarkdownRenderer.svelte'

describe('MarkdownRenderer mermaid rendering', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    renderMarkdown.mockResolvedValue('<p>default</p>')
    mockMermaidRender.mockResolvedValue({ svg: '<svg class="mermaid-svg"><rect/></svg>' })
  })

  it('renders mermaid code blocks as diagrams', async () => {
    renderMarkdown.mockResolvedValue('<pre><code class="language-mermaid">flowchart TD\n  A--&gt;B</code></pre>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '```mermaid\nflowchart TD\n  A-->B\n```' },
    })

    await waitFor(() => {
      const pre = container.querySelector('pre')
      expect(pre).toHaveAttribute('data-mermaid-processed', 'true')
      expect(pre).toHaveClass('mermaid-diagram')
    })

    expect(container.querySelector('svg.mermaid-svg')).toBeInTheDocument()
    expect(mockMermaidRender).toHaveBeenCalledOnce()
  })

  it('leaves non-mermaid code blocks unchanged', async () => {
    renderMarkdown.mockResolvedValue('<pre><code class="language-javascript">const x = 1</code></pre>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '```javascript\nconst x = 1\n```' },
    })

    await waitFor(() => {
      expect(container.querySelector('pre code')?.textContent).toContain('const x = 1')
    })

    expect(container.querySelector('pre')).not.toHaveAttribute('data-mermaid-processed')
    expect(mockMermaidRender).not.toHaveBeenCalled()
  })

  it('preserves original code and shows fallback when mermaid render fails', async () => {
    renderMarkdown.mockResolvedValue('<pre><code class="language-mermaid">flowchart TD\n  A--&gt;B</code></pre>')
    mockMermaidRender.mockRejectedValue(new Error('Parse error'))

    const { container } = render(MarkdownRenderer, {
      props: { source: '```mermaid\nflowchart TD\n  A-->B\n```' },
    })

    await waitFor(() => {
      const pre = container.querySelector('pre')
      expect(pre).toHaveAttribute('data-mermaid-processed', 'true')
      expect(pre?.querySelector('code.language-mermaid')?.textContent).toContain('flowchart TD')
    })

    expect(container.querySelector('.mermaid-error')).toHaveTextContent('Unable to render Mermaid diagram.')
  })

  it('sanitizes rendered mermaid SVG output', async () => {
    renderMarkdown.mockResolvedValue('<pre><code class="language-mermaid">flowchart TD\n  A--&gt;B</code></pre>')
    mockMermaidRender.mockResolvedValue({ svg: '<svg><script>alert(1)</script><rect/></svg>' })

    const { container } = render(MarkdownRenderer, {
      props: { source: '```mermaid\nflowchart TD\n  A-->B\n```' },
    })

    await waitFor(() => {
      expect(container.querySelector('pre')).toHaveAttribute('data-mermaid-processed', 'true')
    })

    expect(DOMPurify.sanitize).toHaveBeenCalledWith(
      '<svg><script>alert(1)</script><rect/></svg>',
      expect.objectContaining({
        USE_PROFILES: { svg: true, svgFilters: true },
      })
    )
  })

  it('does not process the same mermaid block twice on rerender', async () => {
    renderMarkdown.mockResolvedValue('<pre><code class="language-mermaid">flowchart TD\n  A--&gt;B</code></pre>')

    const view = render(MarkdownRenderer, {
      props: { source: '```mermaid\nflowchart TD\n  A-->B\n```', dark: false },
    })

    await waitFor(() => {
      expect(view.container.querySelector('pre')).toHaveAttribute('data-mermaid-processed', 'true')
    })
    expect(mockMermaidRender).toHaveBeenCalledTimes(1)

    await view.rerender({
      source: '```mermaid\nflowchart TD\n  A-->B\n```',
      dark: true,
    })

    await waitFor(() => {
      expect(view.container.querySelector('[data-testid="markdown-content"]')).toHaveClass('th-prose-dark')
    })
    expect(mockMermaidRender).toHaveBeenCalledTimes(1)
  })
})

describe('MarkdownRenderer link navigation', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    renderMarkdown.mockResolvedValue('<p>default</p>')
  })

  it('calls onNavigate with project-relative markdown link', async () => {
    const onNavigate = vi.fn()
    renderMarkdown.mockResolvedValue('<a href="docs/design-brief.md">Design brief</a>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '[Design brief](docs/design-brief.md)', onNavigate },
    })

    await waitFor(() => {
      expect(container.querySelector('a')).toBeInTheDocument()
    })

    const link = container.querySelector('a')
    const allowedDefault = link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))

    expect(allowedDefault).toBe(false)
    expect(onNavigate).toHaveBeenCalledWith('docs/design-brief.md')
    expect(openExternalUrl).not.toHaveBeenCalled()
  })

  it('calls onNavigate with dot-relative markdown link', async () => {
    const onNavigate = vi.fn()
    renderMarkdown.mockResolvedValue('<a href="./foo.md">Foo</a>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '[Foo](./foo.md)', onNavigate },
    })

    await waitFor(() => {
      expect(container.querySelector('a')).toBeInTheDocument()
    })

    const link = container.querySelector('a')
    const allowedDefault = link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))

    expect(allowedDefault).toBe(false)
    expect(onNavigate).toHaveBeenCalledWith('./foo.md')
    expect(openExternalUrl).not.toHaveBeenCalled()
  })

  it('preserves fragment and calls onNavigate for parent-relative markdown link', async () => {
    const onNavigate = vi.fn()
    renderMarkdown.mockResolvedValue('<a href="../bar.md#section">Bar</a>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '[Bar](../bar.md#section)', onNavigate },
    })

    await waitFor(() => {
      expect(container.querySelector('a')).toBeInTheDocument()
    })

    const link = container.querySelector('a')
    const allowedDefault = link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))

    expect(allowedDefault).toBe(false)
    expect(onNavigate).toHaveBeenCalledWith('../bar.md#section')
    expect(openExternalUrl).not.toHaveBeenCalled()
  })

  it('opens https links via openExternalUrl', async () => {
    const onNavigate = vi.fn()
    renderMarkdown.mockResolvedValue('<a href="https://github.com/foo">GitHub</a>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '[GitHub](https://github.com/foo)', onNavigate },
    })

    await waitFor(() => {
      expect(container.querySelector('a')).toBeInTheDocument()
    })

    const link = container.querySelector('a')
    const allowedDefault = link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))

    expect(allowedDefault).toBe(false)
    expect(openExternalUrl).toHaveBeenCalledWith('https://github.com/foo')
    expect(onNavigate).not.toHaveBeenCalled()
  })

  it('opens mailto links via openExternalUrl', async () => {
    const onNavigate = vi.fn()
    renderMarkdown.mockResolvedValue('<a href="mailto:test@test.com">Email</a>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '[Email](mailto:test@test.com)', onNavigate },
    })

    await waitFor(() => {
      expect(container.querySelector('a')).toBeInTheDocument()
    })

    const link = container.querySelector('a')
    const allowedDefault = link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))

    expect(allowedDefault).toBe(false)
    expect(openExternalUrl).toHaveBeenCalledWith('mailto:test@test.com')
    expect(onNavigate).not.toHaveBeenCalled()
  })

  it('does not prevent default for anchor links', async () => {
    const onNavigate = vi.fn()
    renderMarkdown.mockResolvedValue('<a href="#overview">Overview</a>')

    const { container } = render(MarkdownRenderer, {
      props: { source: '[Overview](#overview)', onNavigate },
    })

    await waitFor(() => {
      expect(container.querySelector('a')).toBeInTheDocument()
    })

    const link = container.querySelector('a')
    const allowedDefault = link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))

    expect(allowedDefault).toBe(true)
    expect(onNavigate).not.toHaveBeenCalled()
    expect(openExternalUrl).not.toHaveBeenCalled()
  })

  it('scrolls to target heading after markdown renders when scrollToAnchor is provided', async () => {
    const originalCss = globalThis.CSS
    const originalScrollIntoView = window.HTMLElement.prototype.scrollIntoView
    const scrollSpy = vi.fn()
    globalThis.CSS = { ...(originalCss || {}), escape: (value) => value }
    Object.defineProperty(window.HTMLElement.prototype, 'scrollIntoView', {
      value: scrollSpy,
      configurable: true,
    })

    renderMarkdown.mockResolvedValue('<h2 id="phase-2">Phase 2</h2><p>Details</p>')

    render(MarkdownRenderer, {
      props: {
        source: '## Phase 2\n\nDetails',
        scrollToAnchor: 'phase-2',
      },
    })

    await waitFor(() => {
      expect(scrollSpy).toHaveBeenCalledWith({ behavior: 'smooth' })
    })

    globalThis.CSS = originalCss
    Object.defineProperty(window.HTMLElement.prototype, 'scrollIntoView', {
      value: originalScrollIntoView,
      configurable: true,
    })
  })
})
