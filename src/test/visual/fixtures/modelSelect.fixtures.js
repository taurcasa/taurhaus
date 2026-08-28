const CATALOG = {
  claude: [
    {
      id: 'opus',
      label: 'Opus 5',
      efforts: ['low', 'medium', 'high', 'xhigh', 'max'],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
    {
      id: 'sonnet',
      label: 'Sonnet',
      efforts: ['low', 'medium', 'high', 'xhigh', 'max'],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
  ],
  codex: [
    {
      id: 'gpt-5.6-sol',
      label: 'GPT-5.6-Sol',
      efforts: ['low', 'medium', 'high', 'xhigh', 'max', 'ultra'],
      defaultEffort: 'low',
      deprecated: false,
      replacement: null,
    },
    {
      id: 'gpt-5.6-terra',
      label: 'GPT-5.6-Terra',
      efforts: ['low', 'medium', 'high', 'xhigh', 'max', 'ultra'],
      defaultEffort: 'medium',
      deprecated: false,
      replacement: null,
    },
    {
      id: 'gpt-5.4',
      label: 'GPT-5.4',
      efforts: ['low', 'medium', 'high', 'xhigh'],
      defaultEffort: 'medium',
      deprecated: true,
      replacement: 'gpt-5.6-terra',
    },
  ],
  agy: [
    {
      id: 'gemini-3.7-flash-high',
      label: 'Gemini 3.7 Flash (High)',
      efforts: [],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
  ],
}

function createScenario({ name, theme = 'light', cases }) {
  return { name, theme, catalog: CATALOG, cases }
}

const BASE_CASES = [
  {
    label: 'Known model with efforts',
    tool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: 'xhigh',
  },
  {
    label: 'Catalog default (no model set)',
    tool: 'codex',
    model: '',
    reasoningEffort: null,
  },
  {
    label: 'Inherited effort (unset)',
    tool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: null,
  },
  {
    label: 'Role-declared effort (no default option)',
    tool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: null,
    inheritedEffort: 'high',
  },
  {
    label: 'Custom value from a YAML template',
    tool: 'codex',
    model: 'gpt-6-preview',
    reasoningEffort: 'ultra',
  },
  {
    label: 'Deprecated entry with replacement',
    tool: 'codex',
    model: 'gpt-5.4',
    reasoningEffort: 'high',
  },
  {
    label: 'Tool without efforts',
    tool: 'agy',
    model: 'gemini-3.7-flash-high',
    reasoningEffort: null,
  },
  {
    label: 'Disabled (locked role)',
    tool: 'claude',
    model: 'opus',
    reasoningEffort: 'high',
    disabled: true,
  },
]

export const modelSelectScenarios = [
  createScenario({ name: 'Editor width - light', cases: BASE_CASES }),
  createScenario({ name: 'Editor width - dark', theme: 'dark', cases: BASE_CASES }),
  createScenario({
    name: 'Compact roster row - light',
    cases: BASE_CASES.map((entry) => ({ ...entry, compact: true })),
  }),
  createScenario({
    name: 'Compact roster row - dark',
    theme: 'dark',
    cases: BASE_CASES.map((entry) => ({ ...entry, compact: true })),
  }),
]
