import MarkdownIt from 'markdown-it'
import { fromHighlighter } from '@shikijs/markdown-it/core'
import { createHighlighter } from 'shiki'
import DOMPurify from 'dompurify'

let highlighterPromise = null
const mdInstances = {}
let plainMd = null

/**
 * Lazily create and cache a Shiki highlighter.
 *
 * Uses the full Shiki bundle (~200 languages). This is a desktop app —
 * grammars load from disk once, so bundle size is irrelevant. We never
 * have to manually add languages when viewing new project types.
 */
// Languages that appear frequently in READMEs and project files.
// Pre-loaded at init so the first render doesn't hit lazy-load failures.
const COMMON_LANGS = [
  'bash', 'shell', 'javascript', 'typescript', 'json', 'python',
  'rust', 'html', 'css', 'yaml', 'toml', 'markdown', 'sql', 'diff',
]

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ['github-light', 'github-dark-dimmed'],
      langs: COMMON_LANGS,
    })
  }
  return highlighterPromise
}

/**
 * Ensure a theme is loaded in the highlighter.
 * No-op if the theme is already loaded.
 */
async function ensureThemeLoaded(themeId) {
  const highlighter = await getHighlighter()
  const loaded = highlighter.getLoadedThemes()
  if (!loaded.includes(themeId)) {
    await highlighter.loadTheme(themeId)
  }
}

/**
 * Get a markdown-it instance configured for the given theme ID.
 * Returns a promise that resolves once Shiki is loaded.
 */
async function getMdInstance(themeId) {
  if (mdInstances[themeId]) return mdInstances[themeId]

  const highlighter = await getHighlighter()
  await ensureThemeLoaded(themeId)

  const md = new MarkdownIt({
    html: true,       // Allow raw HTML blocks (common in READMEs: <div>, <img>, etc.)
    linkify: true,
    typographer: false,
  })

  // Disable schema-less auto-linking (e.g. bare "CLAUDE.md" → "http://claude.md/").
  // Full URLs like "https://example.com" in text still auto-link.
  // Proper markdown links [text](url) are unaffected.
  md.linkify.set({ fuzzyLink: false })

  md.use(fromHighlighter(highlighter, {
    theme: themeId,
    defaultLanguage: 'text',
  }))

  mdInstances[themeId] = md
  return md
}

/**
 * Plain markdown-it instance (no Shiki). Used as fallback when the full
 * Shiki pipeline fails — renders markdown without syntax highlighting,
 * which is far better than showing raw text.
 */
function getPlainMd() {
  if (!plainMd) {
    plainMd = new MarkdownIt({ html: true, linkify: true, typographer: false })
    plainMd.linkify.set({ fuzzyLink: false })
  }
  return plainMd
}

/**
 * Render markdown to sanitized HTML.
 *
 * Pipeline:
 *   1. markdown-it (html: true) — parses markdown + raw HTML blocks
 *   2. @shikijs/markdown-it — syntax highlights fenced code blocks
 *   3. DOMPurify — sanitizes output (strips script, onclick, etc.)
 *
 * Falls back to plain markdown-it (no syntax highlighting) if Shiki fails.
 *
 * Image resolution (relative src → base64 data URIs) is handled by the
 * MarkdownRenderer component after render, via the read_project_asset IPC command.
 *
 * @param {string} source — raw markdown text
 * @param {string} theme — Shiki theme ID for syntax highlighting
 * @returns {Promise<string>} sanitized HTML string
 */
export async function renderMarkdown(source, theme = 'github-light') {
  if (!source) return ''

  try {
    // Pre-load languages referenced in fenced code blocks so the
    // markdown-it plugin doesn't throw on unknown languages.
    // Languages that Shiki doesn't support get replaced with 'text'.
    source = await preloadFencedLanguages(source)

    const md = await getMdInstance(theme)
    let raw
    try {
      raw = md.render(source)
    } catch {
      // A language Shiki can't handle slipped through — strip all language
      // hints and retry so the rest of the markdown still renders.
      const safeSource = source.replace(/^```\w[\w+-]*/gm, '```text')
      raw = md.render(safeSource)
    }

    // Shiki uses inline `style` on code spans for syntax coloring — the
    // markdown pipeline includes Shiki output, so we must allow `style`.
    // FORBID_TAGS blocks <style> elements (CSS injection) while keeping
    // inline style= attributes.
    return DOMPurify.sanitize(raw, {
      ADD_TAGS: ['span'],
      ADD_ATTR: ['class', 'style', 'target', 'rel'],
      FORBID_TAGS: ['style'],
    })
  } catch (err) {
    // Shiki pipeline failed entirely — fall back to plain markdown-it.
    // No Shiki output here, so no inline styles needed.
    console.warn(`[markdown] Shiki pipeline failed, using plain fallback: ${err}`)
    const raw = getPlainMd().render(source)
    return DOMPurify.sanitize(raw, {
      ADD_TAGS: ['span'],
      ADD_ATTR: ['class', 'target', 'rel'],
      FORBID_TAGS: ['style'],
    })
  }
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
 * @param {string} theme — Shiki theme ID
 * @returns {Promise<string>} highlighted HTML
 */
export async function highlightCode(code, lang, theme = 'github-light') {
  if (!code) return ''

  const highlighter = await getHighlighter()
  await ensureThemeLoaded(theme)

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
  // Shiki uses inline style= for token colors — must keep `style` in ADD_ATTR.
  // FORBID_TAGS blocks <style> elements while keeping inline style= attributes.
  return DOMPurify.sanitize(html, {
    ADD_TAGS: ['span'],
    ADD_ATTR: ['class', 'style'],
    FORBID_TAGS: ['style'],
  })
}
