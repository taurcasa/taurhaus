import { beforeEach, describe, it, expect, vi } from 'vitest'
import { renderMarkdown, highlightCode } from './markdown.js'

const {
  loadedLangs,
  loadedThemes,
  mockCreateHighlighter,
  mockLoadLanguage,
  mockLoadTheme,
} = vi.hoisted(() => {
  const loadedLangs = new Set()
  const loadedThemes = new Set(['github-light', 'github-dark-dimmed'])
  const mockLoadLanguage = vi.fn((lang) => {
    if (lang === 'brainfuck') return Promise.reject(new Error('not found'))
    loadedLangs.add(lang)
    return Promise.resolve()
  })
  const mockLoadTheme = vi.fn((theme) => {
    loadedThemes.add(theme)
    return Promise.resolve()
  })
  const mockCreateHighlighter = vi.fn((opts = {}) => {
    for (const lang of opts.langs ?? []) {
      loadedLangs.add(String(lang).toLowerCase())
    }
    return Promise.resolve({
      getLoadedLanguages: () => [...loadedLangs],
      getLoadedThemes: () => [...loadedThemes],
      loadLanguage: mockLoadLanguage,
      loadTheme: mockLoadTheme,
      codeToHtml: (code, options) => `<pre class="shiki" data-lang="${options?.lang ?? ''}"><code>${code}</code></pre>`,
    })
  })
  return { loadedLangs, loadedThemes, mockCreateHighlighter, mockLoadLanguage, mockLoadTheme }
})

vi.mock('shiki', () => ({
  createHighlighter: mockCreateHighlighter,
}))

vi.mock('@shikijs/markdown-it/core', () => ({
  fromHighlighter: vi.fn(() => () => {}),
}))

beforeEach(() => {
  mockLoadLanguage.mockClear()
  mockLoadTheme.mockClear()
})

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

  it('accepts theme ID strings for light and dark', async () => {
    const light = await renderMarkdown('hello', 'github-light')
    const dark = await renderMarkdown('hello', 'github-dark-dimmed')
    expect(light).toContain('hello')
    expect(dark).toContain('hello')
  })

  it('loads non-default themes on demand', async () => {
    const html = await renderMarkdown('hello', 'dracula')
    expect(html).toContain('hello')
    expect(mockLoadTheme).toHaveBeenCalledWith('dracula')
  })

  it('preloads fenced code block languages before rendering', async () => {
    loadedLangs.delete('powershell')
    mockLoadLanguage.mockClear()
    // powershell is not in the initial loaded set — should be loaded on demand
    const source = '```powershell\nGet-Process\n```'
    const html = await renderMarkdown(source)
    // Should render without throwing (language was preloaded)
    expect(html).toContain('Get-Process')
    expect(mockLoadLanguage).toHaveBeenCalledWith('powershell')
  })

  it('replaces unknown fenced languages with text', async () => {
    // brainfuck always fails to load in our mock
    const source = '```brainfuck\n+++\n```'
    const html = await renderMarkdown(source)
    // Should still render the code content (as plaintext), not throw
    expect(html).toContain('+++')
  })

  it('renders mermaid fenced blocks with language-mermaid class', async () => {
    const source = '```mermaid\nflowchart TD\n  A-->B\n```'
    const html = await renderMarkdown(source)
    expect(html).toContain('language-mermaid')
    // Mermaid source should remain for downstream diagram rendering.
    expect(html).toContain('flowchart TD')
  })
})

describe('highlightCode', () => {
  it('initializes highlighter with core language set', async () => {
    await highlightCode('const x = 1', 'javascript')
    const opts = mockCreateHighlighter.mock.calls[0]?.[0] ?? {}
    expect(opts.langs).toEqual(
      expect.arrayContaining([
        'javascript', 'typescript', 'json', 'yaml', 'toml', 'markdown',
        'html', 'css', 'rust', 'python', 'bash', 'svelte',
      ])
    )
  })

  it('highlights core languages without lazy language loads', async () => {
    mockLoadLanguage.mockClear()
    const core = [
      'javascript',
      'typescript',
      'rust',
      'python',
      'bash',
      'json',
      'yaml',
      'markdown',
    ]
    for (const lang of core) {
      await highlightCode('const x = 1', lang)
    }
    expect(mockLoadLanguage).not.toHaveBeenCalled()
  })

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

  it('lazy-loads non-core languages on demand', async () => {
    loadedLangs.delete('powershell')
    mockLoadLanguage.mockClear()
    const html = await highlightCode('Get-Process', 'powershell')
    expect(html).toContain('Get-Process')
    expect(mockLoadLanguage).toHaveBeenCalledWith('powershell')
  })

  it('accepts theme ID string', async () => {
    const html = await highlightCode('let x = 1', 'rust', 'github-dark-dimmed')
    expect(html).toContain('let x = 1')
  })
})
