/** Catalog fixture mirroring the backend `ModelCatalog` shape (models/mod.rs). */
export const TEST_MODEL_CATALOG = {
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
    {
      id: 'claude-sonnet-4-6',
      label: 'Claude Sonnet 4.6',
      efforts: ['low', 'medium', 'high'],
      defaultEffort: 'medium',
      deprecated: false,
      replacement: null,
    },
  ],
}
