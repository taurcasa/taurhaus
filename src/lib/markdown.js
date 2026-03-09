import DOMPurify from 'dompurify'

let highlighterPromise = null
let shikiModulesPromise = null
let markdownItCtorPromise = null
const mdInstances = {}
let plainMd = null
const loadedLanguageIds = new Set()
const loadedThemeIds = new Set(['github-light', 'github-dark-dimmed'])
const pendingLanguageLoads = new Map()
const pendingThemeLoads = new Map()
const markdownRenderCache = new Map()
const inflightMarkdownRenders = new Map()
const codeHighlightCache = new Map()
const inflightCodeHighlights = new Map()
const MAX_MARKDOWN_CACHE_ENTRIES = 32
const MAX_CODE_CACHE_ENTRIES = 64

/**
 * Lazily create and cache a Shiki highlighter.
 *
 * Uses the full Shiki bundle (~200 languages). This is a desktop app —
 * grammars load from disk once, so bundle size is irrelevant. We never
 * have to manually add languages when viewing new project types.
 */
// Core languages load eagerly with the highlighter to avoid first-use delay.
// All other languages are lazy-loaded on demand.
const CORE_LANGS = [
  'javascript',
  'typescript',
  'json',
  'yaml',
  'toml',
  'markdown',
  'html',
  'css',
  'rust',
  'python',
  'bash',
  'svelte',
]

const LANG_ALIASES = {
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  ts: 'typescript',
  tsx: 'typescript',
  py: 'python',
  sh: 'bash',
  shell: 'bash',
  zsh: 'bash',
  yml: 'yaml',
  md: 'markdown',
}

for (const lang of CORE_LANGS) {
  loadedLanguageIds.add(normalizeLanguageId(lang))
}

function normalizeLanguageId(lang) {
  const normalized = String(lang ?? '').trim().toLowerCase()
  if (!normalized) return 'text'
  return LANG_ALIASES[normalized] ?? normalized
}

function cacheGet(cache, key) {
  if (!cache.has(key)) return null
  const value = cache.get(key)
  cache.delete(key)
  cache.set(key, value)
  return value
}

function cacheSet(cache, key, value, maxEntries) {
  if (cache.has(key)) {
    cache.delete(key)
  }
  cache.set(key, value)
  while (cache.size > maxEntries) {
    const oldestKey = cache.keys().next().value
    cache.delete(oldestKey)
  }
}

function getMarkdownCacheKey(source, theme) {
  return `${theme}\u0000${source}`
}

function getCodeCacheKey(code, lang, theme) {
  return `${theme}\u0000${normalizeLanguageId(lang)}\u0000${code}`
}

function trackInflight(map, key, factory) {
  if (map.has(key)) return map.get(key)
  const pending = Promise.resolve()
    .then(factory)
    .finally(() => {
      map.delete(key)
    })
  map.set(key, pending)
  return pending
}

async function getShikiModules() {
  if (!shikiModulesPromise) {
    shikiModulesPromise = Promise.all([
      import('shiki'),
      import('@shikijs/markdown-it/core'),
    ]).then(([shikiModule, markdownItModule]) => ({
      createHighlighter: shikiModule.createHighlighter,
      fromHighlighter: markdownItModule.fromHighlighter,
    }))
  }
  return shikiModulesPromise
}

async function getMarkdownItCtor() {
  if (!markdownItCtorPromise) {
    markdownItCtorPromise = import('markdown-it').then((module) => module.default)
  }
  return markdownItCtorPromise
}

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = getShikiModules().then(({ createHighlighter }) =>
      createHighlighter({
        themes: ['github-light', 'github-dark-dimmed'],
        langs: CORE_LANGS,
      })
    )
  }
  return highlighterPromise
}

/**
 * Ensure a theme is loaded in the highlighter.
 * No-op if the theme is already loaded.
 */
async function ensureThemeLoaded(themeId) {
  const highlighter = await getHighlighter()
  if (loadedThemeIds.has(themeId)) return highlighter
  if (!pendingThemeLoads.has(themeId)) {
    pendingThemeLoads.set(
      themeId,
      highlighter.loadTheme(themeId).then(() => {
        loadedThemeIds.add(themeId)
      }).finally(() => {
        pendingThemeLoads.delete(themeId)
      })
    )
  }
  await pendingThemeLoads.get(themeId)
  return highlighter
}

async function ensureLanguageLoaded(lang) {
  const normalizedLang = normalizeLanguageId(lang)
  if (loadedLanguageIds.has(normalizedLang) || normalizedLang === 'text') {
    return normalizedLang
  }

  const highlighter = await getHighlighter()
  if (!pendingLanguageLoads.has(normalizedLang)) {
    pendingLanguageLoads.set(
      normalizedLang,
      highlighter.loadLanguage(normalizedLang).then(() => {
        loadedLanguageIds.add(normalizedLang)
        return normalizedLang
      }).catch(() => null).finally(() => {
        pendingLanguageLoads.delete(normalizedLang)
      })
    )
  }

  const loaded = await pendingLanguageLoads.get(normalizedLang)
  return loaded ?? null
}

/**
 * Get a markdown-it instance configured for the given theme ID.
 * Returns a promise that resolves once Shiki is loaded.
 */
async function getMdInstance(themeId) {
  if (mdInstances[themeId]) return mdInstances[themeId]

  const { fromHighlighter } = await getShikiModules()
  const MarkdownIt = await getMarkdownItCtor()
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
async function getPlainMd() {
  if (!plainMd) {
    const MarkdownIt = await getMarkdownItCtor()
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
  const cacheKey = getMarkdownCacheKey(source, theme)
  const cached = cacheGet(markdownRenderCache, cacheKey)
  if (cached) return cached

  return trackInflight(inflightMarkdownRenders, cacheKey, async () => {
    let rendered
    try {
      // Pre-load languages referenced in fenced code blocks so the
      // markdown-it plugin doesn't throw on unknown languages.
      // Languages that Shiki doesn't support get replaced with 'text'.
      const preparedSource = await preloadFencedLanguages(source)

      const md = await getMdInstance(theme)
      let raw
      try {
        raw = md.render(preparedSource)
      } catch {
        // A language Shiki can't handle slipped through — strip all language
        // hints and retry so the rest of the markdown still renders.
        const safeSource = preparedSource.replace(/^```\w[\w+-]*/gm, '```text')
        raw = md.render(safeSource)
      }

      // Shiki uses inline `style` on code spans for syntax coloring — the
      // markdown pipeline includes Shiki output, so we must allow `style`.
      // FORBID_TAGS blocks <style> elements (CSS injection) while keeping
      // inline style= attributes.
      rendered = DOMPurify.sanitize(raw, {
        ADD_TAGS: ['span'],
        ADD_ATTR: ['class', 'style', 'target', 'rel'],
        FORBID_TAGS: ['style'],
      })
    } catch (err) {
      // Shiki pipeline failed entirely — fall back to plain markdown-it.
      // No Shiki output here, so no inline styles needed.
      console.warn(`[markdown] Shiki pipeline failed, using plain fallback: ${err}`)
      const raw = (await getPlainMd()).render(source)
      rendered = DOMPurify.sanitize(raw, {
        ADD_TAGS: ['span'],
        ADD_ATTR: ['class', 'target', 'rel'],
        FORBID_TAGS: ['style'],
      })
    }

    cacheSet(markdownRenderCache, cacheKey, rendered, MAX_MARKDOWN_CACHE_ENTRIES)
    return rendered
  })
}

/**
 * Scan markdown source for fenced code block language hints (```lang),
 * load each one into Shiki on demand, and replace any that don't exist
 * with 'text' so the markdown-it plugin doesn't throw.
 */
async function preloadFencedLanguages(source) {
  // Collect all unique language hints from fenced code blocks
  const langRegex = /^```(\w[\w+-]*)/gm
  const pendingLoads = new Map()
  const unknown = new Set()
  let match
  while ((match = langRegex.exec(source)) !== null) {
    const lang = match[1]
    const normalizedLang = normalizeLanguageId(lang)
    if (pendingLoads.has(lang)) continue

    if (loadedLanguageIds.has(normalizedLang) || normalizedLang === 'text') {
      pendingLoads.set(lang, Promise.resolve(normalizedLang))
      continue
    }

    pendingLoads.set(
      lang,
      ensureLanguageLoaded(normalizedLang).then((loadedLang) => {
        if (!loadedLang) unknown.add(lang)
        return loadedLang
      })
    )
  }

  if (pendingLoads.size > 0) {
    try {
      await Promise.all(pendingLoads.values())
    } catch {
      // Individual loads are converted to null above; Promise.all should not reject.
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
  const cacheKey = getCodeCacheKey(code, lang, theme)
  const cached = cacheGet(codeHighlightCache, cacheKey)
  if (cached) return cached

  return trackInflight(inflightCodeHighlights, cacheKey, async () => {
    const highlighter = await ensureThemeLoaded(theme)

    // Load the language on demand if not already loaded
    const normalizedLang = normalizeLanguageId(lang)
    let effectiveLang = normalizedLang || 'text'
    if (effectiveLang !== 'text') {
      const loadedLang = await ensureLanguageLoaded(effectiveLang)
      if (!loadedLang) {
        // Language not available in Shiki — fall back to plaintext
        effectiveLang = 'text'
      }
    }

    const html = highlighter.codeToHtml(code, { lang: effectiveLang, theme })
    const sanitized = DOMPurify.sanitize(html, {
      ADD_TAGS: ['span'],
      ADD_ATTR: ['class', 'style'],
      FORBID_TAGS: ['style'],
    })
    cacheSet(codeHighlightCache, cacheKey, sanitized, MAX_CODE_CACHE_ENTRIES)
    return sanitized
  })
}

export function markdownHasMermaidFence(source) {
  return /^```mermaid(?:\s|$)/gim.test(String(source ?? ''))
}
