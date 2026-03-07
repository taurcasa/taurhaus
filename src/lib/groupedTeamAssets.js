const TOOL_ORDER = ['claude', 'codex', 'gemini']

const GROUPED_TEAM_ASSETS = {
  'claude-codex': {
    key: 'claude-codex',
    maskUrl: new URL('./assets/grouped-icons/claude-codex.png', import.meta.url).href,
    railWidthPx: 27,
    stackWidthPx: 24,
  },
  'claude-gemini': {
    key: 'claude-gemini',
    maskUrl: new URL('./assets/grouped-icons/claude-gemini.png', import.meta.url).href,
    railWidthPx: 27,
    stackWidthPx: 24,
  },
  'codex-gemini': {
    key: 'codex-gemini',
    maskUrl: new URL('./assets/grouped-icons/codex-gemini.png', import.meta.url).href,
    railWidthPx: 27,
    stackWidthPx: 24,
  },
  'claude-codex-gemini': {
    key: 'claude-codex-gemini',
    maskUrl: new URL('./assets/grouped-icons/claude-codex-gemini.png', import.meta.url).href,
    railWidthPx: 39,
    stackWidthPx: 34,
  },
}

function orderedUniqueTools(tools) {
  const list = Array.isArray(tools) ? tools.filter(Boolean) : []
  const seen = new Set()
  const ordered = []

  for (const tool of TOOL_ORDER) {
    if (list.includes(tool)) {
      seen.add(tool)
      ordered.push(tool)
    }
  }

  for (const tool of list) {
    if (seen.has(tool)) continue
    seen.add(tool)
    ordered.push(tool)
  }

  return ordered
}

export function getGroupedTeamAsset(tools) {
  const ordered = orderedUniqueTools(tools)
  if (ordered.length < 2) return null
  return GROUPED_TEAM_ASSETS[ordered.join('-')] ?? null
}
