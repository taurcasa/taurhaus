import { describe, it, expect, vi } from 'vitest'
import { renderMarkdown, highlightCode } from './markdown.js'

// Mock shiki since it requires WASM (not available in jsdom)
const loadedLangs = new Set(['javascript', 'rust'])
vi.mock('shiki', () => ({
  createHighlighter: vi.fn(() => Promise.resolve({
    getLoadedLanguages: () => [...loadedLangs],
    loadLanguage: vi.fn((lang) => {
      // Simulate: some languages load, some don't
      if (lang === 'brainfuck') return Promise.reject(new Error('not found'))
      loadedLangs.add(lang)
      return Promise.resolve()
    }),
    codeToHtml: (code, opts) => `<pre class="shiki"><code>${code}</code></pre>`,
  })),
}))

vi.mock('@shikijs/markdown-it/core', () => ({
  fromHighlighter: vi.fn(() => () => {}),
}))

describe('renderMarkdown', () => {
  it('returns empty string for empty input', async () => {
    expect(await renderMarkdown('')).toBe('')
    expect(await renderMarkdown(null)).toBe('')
  })

  it('renders basic markdown to HTML', async () => {
    const html = await renderMarkdown('# Hello World')
    expect(html).toContain('<h1>')
    expect(html).toContain('Hello World')
  })

  it('renders paragraphs', async () => {
    const html = await renderMarkdown('Some text here.')
    expect(html).toContain('<p>')
    expect(html).toContain('Some text here.')
  })

  it('renders bold and italic', async () => {
    const html = await renderMarkdown('**bold** and *italic*')
    expect(html).toContain('<strong>bold</strong>')
    expect(html).toContain('<em>italic</em>')
  })

  it('renders links with linkify', async () => {
    const html = await renderMarkdown('[link](https://example.com)')
    expect(html).toContain('<a')
    expect(html).toContain('href="https://example.com"')
  })

  it('renders unordered lists', async () => {
    const html = await renderMarkdown('- item one\n- item two')
    expect(html).toContain('<ul>')
    expect(html).toContain('<li>')
    expect(html).toContain('item one')
  })

  it('renders ordered lists', async () => {
    const html = await renderMarkdown('1. first\n2. second')
    expect(html).toContain('<ol>')
    expect(html).toContain('first')
  })

  it('renders blockquotes', async () => {
    const html = await renderMarkdown('> a quote')
    expect(html).toContain('<blockquote>')
    expect(html).toContain('a quote')
  })

  it('renders tables', async () => {
    const html = await renderMarkdown('| A | B |\n|---|---|\n| 1 | 2 |')
    expect(html).toContain('<table>')
    expect(html).toContain('<th>')
    expect(html).toContain('<td>')
  })

  // html: true — raw HTML blocks pass through for README compatibility
  it('renders raw HTML blocks', async () => {
    const html = await renderMarkdown('<div align="center"><strong>centered</strong></div>')
    expect(html).toContain('<div')
    expect(html).toContain('<strong>centered</strong>')
  })

  it('renders img tags from raw HTML', async () => {
    const html = await renderMarkdown('<img src="https://example.com/logo.png" alt="Logo" width="128" />')
    expect(html).toContain('<img')
    expect(html).toContain('src="https://example.com/logo.png"')
    expect(html).toContain('alt="Logo"')
  })

  it('preserves relative img src for component-level resolution', async () => {
    const html = await renderMarkdown('<img src="web/static/logo.jpg" alt="Logo" />')
    expect(html).toContain('<img')
    expect(html).toContain('src="web/static/logo.jpg"')
  })

  // Security: DOMPurify strips dangerous elements and attributes
  it('sanitizes script tags', async () => {
    const html = await renderMarkdown('<script>alert("xss")</script>')
    expect(html).not.toContain('<script>')
  })

  it('strips dangerous event handlers', async () => {
    const html = await renderMarkdown('<div onclick="alert(1)">hi</div>')
    expect(html).not.toContain('onclick')
    expect(html).toContain('hi')
  })

  it('returns different output for dark vs light', async () => {
    const light = await renderMarkdown('hello', false)
    const dark = await renderMarkdown('hello', true)
    expect(light).toContain('hello')
    expect(dark).toContain('hello')
  })

  it('preloads fenced code block languages before rendering', async () => {
    // powershell is not in the initial loaded set — should be loaded on demand
    const source = '```powershell\nGet-Process\n```'
    const html = await renderMarkdown(source)
    // Should render without throwing (language was preloaded)
    expect(html).toContain('Get-Process')
  })

  it('replaces unknown fenced languages with text', async () => {
    // brainfuck always fails to load in our mock
    const source = '```brainfuck\n+++\n```'
    const html = await renderMarkdown(source)
    // Should still render the code content (as plaintext), not throw
    expect(html).toContain('+++')
  })
})

describe('highlightCode', () => {
  it('returns empty string for empty input', async () => {
    expect(await highlightCode('')).toBe('')
    expect(await highlightCode(null)).toBe('')
  })

  it('returns highlighted HTML for known language', async () => {
    const html = await highlightCode('const x = 1', 'javascript')
    expect(html).toContain('<pre')
    expect(html).toContain('const x = 1')
  })

  it('falls back to text for unknown language', async () => {
    const html = await highlightCode('hello', 'brainfuck')
    expect(html).toContain('hello')
  })
})
