import MarkdownIt from 'markdown-it'
import { fromHighlighter } from '@shikijs/markdown-it/core'
import { createHighlighter } from 'shiki'
import DOMPurify from 'dompurify'

let highlighterPromise = null
let mdInstances = { light: null, dark: null }

/**
 * Lazily create and cache a Shiki highlighter.
 *
 * Uses the full Shiki bundle (~200 languages). This is a desktop app —
 * grammars load from disk once, so bundle size is irrelevant. We never
 * have to manually add languages when viewing new project types.
 */
function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ['github-light', 'github-dark-dimmed'],
      langs: [],  // start empty — loaded on demand by the full bundle
    })
  }
  return highlighterPromise
}

/**
 * Get a markdown-it instance configured for the given theme.
 * Returns a promise that resolves once Shiki is loaded.
 */
async function getMdInstance(theme) {
  const key = theme === 'dark' ? 'dark' : 'light'
  if (mdInstances[key]) return mdInstances[key]

  const highlighter = await getHighlighter()

  const md = new MarkdownIt({
    html: true,       // Allow raw HTML blocks (common in READMEs: <div>, <img>, etc.)
    linkify: true,
    typographer: false,
  })

  md.use(fromHighlighter(highlighter, {
    theme: theme === 'dark' ? 'github-dark-dimmed' : 'github-light',
    defaultLanguage: 'text',
  }))

  mdInstances[key] = md
  return md
}

/**
 * Render markdown to sanitized HTML.
 *
 * Pipeline:
 *   1. markdown-it (html: true) — parses markdown + raw HTML blocks
 *   2. @shikijs/markdown-it — syntax highlights fenced code blocks
 *   3. DOMPurify — sanitizes output (strips script, onclick, etc.)
 *
 * Image resolution (relative src → base64 data URIs) is handled by the
 * MarkdownRenderer component after render, via the read_project_asset IPC command.
 *
 * @param {string} source — raw markdown text
 * @param {boolean} isDark — use dark theme for syntax highlighting
 * @returns {Promise<string>} sanitized HTML string
 */
export async function renderMarkdown(source, isDark = false) {
  if (!source) return ''

  // Pre-load languages referenced in fenced code blocks so the
  // markdown-it plugin doesn't throw on unknown languages.
  // Languages that Shiki doesn't support get replaced with 'text'.
  source = await preloadFencedLanguages(source)

  const md = await getMdInstance(isDark ? 'dark' : 'light')
  const raw = md.render(source)

  return DOMPurify.sanitize(raw, {
    ADD_TAGS: ['span'],
    ADD_ATTR: ['class', 'style'],
  })
}

/**
 * Scan markdown source for fenced code block language hints (```lang),
 * load each one into Shiki on demand, and replace any that don't exist
 * with 'text' so the markdown-it plugin doesn't throw.
 */
async function preloadFencedLanguages(source) {
  const highlighter = await getHighlighter()
  const loaded = new Set(highlighter.getLoadedLanguages())

  // Collect all unique language hints from fenced code blocks
  const langRegex = /^```(\w[\w+-]*)/gm
  const seen = new Set()
  const unknown = new Set()
  let match
  while ((match = langRegex.exec(source)) !== null) {
    const lang = match[1]
    if (seen.has(lang)) continue
    seen.add(lang)

    if (!loaded.has(lang.toLowerCase())) {
      try {
        await highlighter.loadLanguage(lang.toLowerCase())
      } catch {
        unknown.add(lang)
      }
    }
  }

  // Replace unknown language hints with 'text'
  for (const lang of unknown) {
    source = source.replaceAll('```' + lang, '```text')
  }
  return source
}

/**
 * Highlight a single code string (for the Files tab code viewer).
 * Falls back to plaintext for unknown languages.
 *
 * @param {string} code — source code
 * @param {string} lang — language identifier
 * @param {boolean} isDark — use dark theme
 * @returns {Promise<string>} highlighted HTML
 */
export async function highlightCode(code, lang, isDark = false) {
  if (!code) return ''

  const highlighter = await getHighlighter()
  const theme = isDark ? 'github-dark-dimmed' : 'github-light'

  // Load the language on demand if not already loaded
  const loadedLangs = highlighter.getLoadedLanguages()
  if (lang && !loadedLangs.includes(lang)) {
    try {
      await highlighter.loadLanguage(lang)
    } catch {
      // Language not available in Shiki — fall back to plaintext
      lang = 'text'
    }
  }

  const effectiveLang = lang || 'text'
  const html = highlighter.codeToHtml(code, { lang: effectiveLang, theme })
  return DOMPurify.sanitize(html, {
    ADD_TAGS: ['span'],
    ADD_ATTR: ['class', 'style'],
  })
}
