import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'
import { createBundledHighlighter } from 'shiki/core'
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript'
import { bundledThemes } from 'shiki/themes'

const SUPPORTED_LANGUAGE_LOADERS = {
  bash: () => import('@shikijs/langs/bash'),
  css: () => import('@shikijs/langs/css'),
  diff: () => import('@shikijs/langs/diff'),
  dockerfile: () => import('@shikijs/langs/dockerfile'),
  graphql: () => import('@shikijs/langs/graphql'),
  html: () => import('@shikijs/langs/html'),
  javascript: () => import('@shikijs/langs/javascript'),
  json: () => import('@shikijs/langs/json'),
  json5: () => import('@shikijs/langs/json5'),
  jsonc: () => import('@shikijs/langs/jsonc'),
  markdown: () => import('@shikijs/langs/markdown'),
  mdx: () => import('@shikijs/langs/mdx'),
  mermaid: () => import('@shikijs/langs/mermaid'),
  powershell: () => import('@shikijs/langs/powershell'),
  python: () => import('@shikijs/langs/python'),
  rust: () => import('@shikijs/langs/rust'),
  shellsession: () => import('@shikijs/langs/shellsession'),
  sql: () => import('@shikijs/langs/sql'),
  svelte: () => import('@shikijs/langs/svelte'),
  toml: () => import('@shikijs/langs/toml'),
  tsx: () => import('@shikijs/langs/tsx'),
  typescript: () => import('@shikijs/langs/typescript'),
  xml: () => import('@shikijs/langs/xml'),
  yaml: () => import('@shikijs/langs/yaml'),
}

const LANG_ALIASES = {
  cjs: 'javascript',
  docker: 'dockerfile',
  js: 'javascript',
  jsx: 'tsx',
  md: 'markdown',
  mjs: 'javascript',
  mmd: 'mermaid',
  py: 'python',
  shell: 'bash',
  sh: 'bash',
  ts: 'typescript',
  yml: 'yaml',
  zsh: 'bash',
}

const CORE_LANGS = [
  'bash',
  'javascript',
  'json',
  'markdown',
  'mermaid',
  'python',
  'rust',
  'svelte',
  'typescript',
]

const MAX_MARKDOWN_SHIKI_CHARS = 120_000
const MAX_MARKDOWN_SHIKI_LINES = 2_000
const MAX_CODE_HIGHLIGHT_CHARS = 80_000
const MAX_CODE_HIGHLIGHT_LINES = 2_000
const MAX_MARKDOWN_CACHE_ENTRIES = 24
const MAX_MARKDOWN_CACHE_SOURCE_CHARS = 60_000
const MAX_CODE_CACHE_ENTRIES = 48
const MAX_CODE_CACHE_SOURCE_CHARS = 24_000

let highlighterPromise = null
const mdInstances = {}
let plainMd = null
const markdownRenderCache = new Map()
const codeHighlightCache = new Map()

const createHighlighter = createBundledHighlighter({
  langs: SUPPORTED_LANGUAGE_LOADERS,
  themes: bundledThemes,
  engine: () => createJavaScriptRegexEngine(),
})

function normalizeLanguageId(lang) {
  const normalized = String(lang ?? '').trim().toLowerCase()
  if (!normalized) return 'text'
  return LANG_ALIASES[normalized] ?? normalized
}

function cacheGet(map, key) {
  if (!map.has(key)) return null
  const value = map.get(key)
  map.delete(key)
  map.set(key, value)
  return value
}

function cacheSet(map, key, value, maxEntries) {
  if (map.has(key)) map.delete(key)
  map.set(key, value)
  while (map.size > maxEntries) {
    const oldestKey = map.keys().next().value
    map.delete(oldestKey)
  }
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}

function countLines(value) {
  if (!value) return 0
  let lines = 1
  for (let i = 0; i < value.length; i += 1) {
    if (value[i] === '\n') lines += 1
  }
  return lines
}

function shouldCacheSource(source, maxChars) {
  return typeof source === 'string' && source.length <= maxChars
}

function shouldUseMarkdownShiki(source) {
  return source.length <= MAX_MARKDOWN_SHIKI_CHARS && countLines(source) <= MAX_MARKDOWN_SHIKI_LINES
}

function shouldHighlightCode(code) {
  return code.length <= MAX_CODE_HIGHLIGHT_CHARS && countLines(code) <= MAX_CODE_HIGHLIGHT_LINES
}

function normalizeLoadedLanguageSet(highlighter) {
  return new Set(highlighter.getLoadedLanguages().map(normalizeLanguageId))
}

function isSupportedLanguage(lang) {
  return Object.hasOwn(SUPPORTED_LANGUAGE_LOADERS, lang)
}

function renderPlainFence(code, lang) {
  const languageClass = lang && lang !== 'text' ? ` class="language-${escapeHtml(lang)}"` : ''
  return `<pre><code${languageClass}>${escapeHtml(code)}</code></pre>`
}

function renderHighlightedFence(code, lang, themeId, highlighter) {
  const normalizedLang = normalizeLanguageId(lang)
  if (normalizedLang === 'mermaid') {
    return renderPlainFence(code, 'mermaid')
  }

  const loadedLangs = normalizeLoadedLanguageSet(highlighter)
  const effectiveLang = loadedLangs.has(normalizedLang) ? normalizedLang : 'text'
  if (effectiveLang === 'text') {
    return renderPlainFence(code, normalizedLang)
  }

  return highlighter.codeToHtml(code, {
    lang: effectiveLang,
    theme: themeId,
  })
}

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ['github-light', 'github-dark-dimmed'],
      langs: CORE_LANGS,
    })
  }
  return highlighterPromise
}

async function ensureThemeLoaded(themeId) {
  const highlighter = await getHighlighter()
  const loadedThemes = highlighter.getLoadedThemes()
  if (!loadedThemes.includes(themeId)) {
    await highlighter.loadTheme(themeId)
  }
}

async function ensureLanguageLoaded(lang) {
  const normalizedLang = normalizeLanguageId(lang)
  if (!isSupportedLanguage(normalizedLang) || normalizedLang === 'mermaid') {
    return false
  }

  const highlighter = await getHighlighter()
  const loadedLangs = normalizeLoadedLanguageSet(highlighter)
  if (loadedLangs.has(normalizedLang)) {
    return true
  }

  try {
    await highlighter.loadLanguage(normalizedLang)
    return true
  } catch {
    return false
  }
}

async function getMdInstance(themeId) {
  if (mdInstances[themeId]) return mdInstances[themeId]

  const highlighter = await getHighlighter()
  await ensureThemeLoaded(themeId)

  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: false,
    highlight(code, lang) {
      return renderHighlightedFence(code, lang, themeId, highlighter)
    },
  })

  md.linkify.set({ fuzzyLink: false })
  mdInstances[themeId] = md
  return md
}

async function getPlainMd() {
  if (!plainMd) {
    plainMd = new MarkdownIt({ html: true, linkify: true, typographer: false })
    plainMd.linkify.set({ fuzzyLink: false })
  }
  return plainMd
}

function getFencedLanguages(source) {
  const langRegex = /^```(\w[\w+-]*)/gm
  const langs = new Set()
  let match
  while ((match = langRegex.exec(source)) !== null) {
    langs.add(normalizeLanguageId(match[1]))
  }
  return [...langs]
}

async function preloadMarkdownLanguages(source) {
  const langs = getFencedLanguages(source).filter((lang) => lang !== 'text' && lang !== 'mermaid' && isSupportedLanguage(lang))
  if (langs.length === 0) return
  await Promise.all(langs.map((lang) => ensureLanguageLoaded(lang)))
}

function sanitizeRenderedHtml(raw, allowInlineStyles) {
  return DOMPurify.sanitize(raw, {
    ADD_TAGS: ['span'],
    ADD_ATTR: allowInlineStyles
      ? ['class', 'style', 'target', 'rel']
      : ['class', 'target', 'rel'],
    FORBID_TAGS: ['style'],
  })
}

export function markdownHasMermaidFence(source) {
  return /(^|\n)```(?:mermaid|mmd)\b/i.test(String(source ?? ''))
}

export async function renderMarkdown(source, theme = 'github-light') {
  if (!source) return ''

  const cacheKey = `${theme}\u0000${source}`
  if (shouldCacheSource(source, MAX_MARKDOWN_CACHE_SOURCE_CHARS)) {
    const cached = cacheGet(markdownRenderCache, cacheKey)
    if (cached !== null) return cached
  }

  try {
    const useShiki = shouldUseMarkdownShiki(source)
    let raw

    if (useShiki) {
      await preloadMarkdownLanguages(source)
      raw = (await getMdInstance(theme)).render(source)
    } else {
      raw = (await getPlainMd()).render(source)
    }

    const sanitized = sanitizeRenderedHtml(raw, useShiki)
    if (shouldCacheSource(source, MAX_MARKDOWN_CACHE_SOURCE_CHARS)) {
      cacheSet(markdownRenderCache, cacheKey, sanitized, MAX_MARKDOWN_CACHE_ENTRIES)
    }
    return sanitized
  } catch (err) {
    console.warn(`[markdown] Shiki pipeline failed, using plain fallback: ${err}`)
    const raw = (await getPlainMd()).render(source)
    const sanitized = sanitizeRenderedHtml(raw, false)
    if (shouldCacheSource(source, MAX_MARKDOWN_CACHE_SOURCE_CHARS)) {
      cacheSet(markdownRenderCache, cacheKey, sanitized, MAX_MARKDOWN_CACHE_ENTRIES)
    }
    return sanitized
  }
}

export async function highlightCode(code, lang, theme = 'github-light') {
  if (!code) return ''
  if (!shouldHighlightCode(code)) return ''

  const normalizedLang = normalizeLanguageId(lang)
  const cacheKey = `${theme}\u0000${normalizedLang}\u0000${code}`
  if (shouldCacheSource(code, MAX_CODE_CACHE_SOURCE_CHARS)) {
    const cached = cacheGet(codeHighlightCache, cacheKey)
    if (cached !== null) return cached
  }

  const highlighter = await getHighlighter()
  await ensureThemeLoaded(theme)

  let effectiveLang = normalizedLang || 'text'
  if (!isSupportedLanguage(effectiveLang) || effectiveLang === 'mermaid') {
    effectiveLang = 'text'
  } else {
    const loadedLangs = normalizeLoadedLanguageSet(highlighter)
    if (!loadedLangs.has(effectiveLang)) {
      const loaded = await ensureLanguageLoaded(effectiveLang)
      if (!loaded) {
        effectiveLang = 'text'
      }
    }
  }

  const html = highlighter.codeToHtml(code, { lang: effectiveLang, theme })
  const sanitized = DOMPurify.sanitize(html, {
    ADD_TAGS: ['span'],
    ADD_ATTR: ['class', 'style'],
    FORBID_TAGS: ['style'],
  })

  if (shouldCacheSource(code, MAX_CODE_CACHE_SOURCE_CHARS)) {
    cacheSet(codeHighlightCache, cacheKey, sanitized, MAX_CODE_CACHE_ENTRIES)
  }
  return sanitized
}
