import MarkdownIt from 'markdown-it'
import { fromHighlighter } from '@shikijs/markdown-it/core'
import { createHighlighterCore } from 'shiki/core'
import { createOnigurumaEngine } from 'shiki/engine/oniguruma'
import DOMPurify from 'dompurify'

let highlighterPromise = null
let mdInstances = { light: null, dark: null }

/**
 * Lazily create and cache a Shiki highlighter.
 * The highlighter loads TextMate grammars on demand.
 */
function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighterCore({
      themes: [
        import('shiki/themes/github-light.mjs'),
        import('shiki/themes/github-dark-dimmed.mjs'),
      ],
      langs: [
        import('shiki/langs/javascript.mjs'),
        import('shiki/langs/typescript.mjs'),
        import('shiki/langs/rust.mjs'),
        import('shiki/langs/python.mjs'),
        import('shiki/langs/bash.mjs'),
        import('shiki/langs/shell.mjs'),
        import('shiki/langs/json.mjs'),
        import('shiki/langs/yaml.mjs'),
        import('shiki/langs/toml.mjs'),
        import('shiki/langs/html.mjs'),
        import('shiki/langs/css.mjs'),
        import('shiki/langs/svelte.mjs'),
        import('shiki/langs/markdown.mjs'),
        import('shiki/langs/sql.mjs'),
        import('shiki/langs/diff.mjs'),
      ],
      engine: createOnigurumaEngine(import('shiki/wasm')),
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
 * @param {string} code — source code
 * @param {string} lang — language identifier
 * @param {boolean} isDark — use dark theme
 * @returns {Promise<string>} highlighted HTML
 */
export async function highlightCode(code, lang, isDark = false) {
  if (!code) return ''

  const highlighter = await getHighlighter()
  const theme = isDark ? 'github-dark-dimmed' : 'github-light'

  // Check if language is loaded, fall back to plaintext
  const loadedLangs = highlighter.getLoadedLanguages()
  const effectiveLang = loadedLangs.includes(lang) ? lang : 'text'

  const html = highlighter.codeToHtml(code, { lang: effectiveLang, theme })
  return DOMPurify.sanitize(html, {
    ADD_TAGS: ['span'],
    ADD_ATTR: ['class', 'style'],
  })
}
