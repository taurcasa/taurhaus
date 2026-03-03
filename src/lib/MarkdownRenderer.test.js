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
