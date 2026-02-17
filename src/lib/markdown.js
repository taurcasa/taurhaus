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

  const md = await getMdInstance(isDark ? 'dark' : 'light')
  const raw = md.render(source)

  return DOMPurify.sanitize(raw, {
    ADD_TAGS: ['span'],
    ADD_ATTR: ['class', 'style'],
  })
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
