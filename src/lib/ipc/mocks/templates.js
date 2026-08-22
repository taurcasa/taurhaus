export const MOCK_ROLE_TEMPLATES = [
  {
    roleId: 'claude-orchestrator',
    name: 'Claude Orchestrator',
    version: '1.0.0',
    kind: 'lead',
    cliTool: 'claude',
    model: 'claude-opus-4-6',
    defaultNamePattern: 'lead-{project}',
    focusArea: 'Team orchestration',
    contextSummary: 'Keeps the team aligned on sequencing, blockers, and delivery quality.',
    behaviorSummary: 'Coordinates specialists and avoids taking over implementation lanes.',
    capabilities: ['planning', 'coordination', 'review', 'triage'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Coordinate team execution, assign scoped tasks, track blockers, and synthesize outcomes for the user.',
    behavioralContract: {
      communication: [
        'Acknowledge requests quickly and classify next action.',
        'Assign owners with acceptance criteria and expected evidence.',
      ],
      execution: [
        'Keep tasks scoped and verify completion evidence before closure.',
        'Enforce project conventions and quality gates.',
      ],
      escalation: [
        'Escalate blockers with context and options.',
        'Do not allow blocked work to stall silently.',
      ],
    },
    constraints: {
      minInstances: 1,
      maxInstances: 1,
      requiresLeadTool: null,
      allowedProjectBinding: 'lead_project',
    },
  },
  {
    roleId: 'codex-orchestrator',
    name: 'Codex Orchestrator',
    version: '1.0.0',
    kind: 'lead',
    cliTool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: 'high',
    defaultNamePattern: 'lead-{project}',
    focusArea: 'Execution orchestration',
    contextSummary: 'Keeps the active implementation plan, verification state, and blockers aligned across the team.',
    behaviorSummary: 'Coordinates delivery sequencing and escalates product-direction changes instead of broadening scope.',
    capabilities: ['planning', 'coordination', 'implementation'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Coordinate scoped execution, keep the implementation plan current, and unblock delivery without taking over specialist lanes.',
    behavioralContract: {
      communication: [
        'Restate the execution plan before delegating work.',
        'Keep status updates concrete and evidence-based.',
      ],
      execution: [
        'Sequence work so dependencies stay unblocked.',
        'Verify implementation evidence before closing tasks.',
      ],
      escalation: [
        'Escalate product or architecture direction changes immediately.',
      ],
    },
    constraints: {
      minInstances: 1,
      maxInstances: 1,
      requiresLeadTool: null,
      allowedProjectBinding: 'lead_project',
    },
  },
  {
    roleId: 'codex-developer',
    name: 'Codex Developer',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: 'high',
    defaultNamePattern: 'dev-{n}',
    focusArea: 'Scoped implementation',
    contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
    behaviorSummary: 'Implements narrowly and escalates blockers instead of broadening scope.',
    capabilities: ['implementation', 'testing', 'debugging'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Implement assigned scope with TDD where applicable, keep changes focused, and report verification steps.',
    behavioralContract: {
      communication: [
        'Acknowledge assignment and restate scope before editing.',
        'Provide concise progress updates on longer tasks.',
      ],
      execution: [
        'Keep edits scoped to assigned work.',
        'Write/update tests for behavior changes.',
      ],
      escalation: [
        'Escalate blockers immediately with attempted fixes.',
        'Flag unexpected repo state before continuing.',
      ],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 8,
      requiresLeadTool: 'claude',
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'codex-architect',
    name: 'Codex Architect',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: 'high',
    defaultNamePattern: 'architect-{n}',
    focusArea: 'Architecture decisions and structural review',
    contextSummary: 'Carries long-lived context around module boundaries, tradeoffs, and review history.',
    behaviorSummary: 'Handles structural decisions and escalates product-direction changes.',
    capabilities: ['architecture', 'review'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Own structural review, module boundaries, and long-lived architectural context for the team.',
    behavioralContract: {
      communication: ['State architectural tradeoffs clearly.'],
      execution: ['Prefer minimal structural change that solves the real problem.'],
      escalation: ['Escalate direction changes that alter boundaries or scope.'],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 4,
      requiresLeadTool: null,
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'codex-qa',
    name: 'Codex QA Engineer',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'codex',
    model: 'gpt-5.6-terra',
    reasoningEffort: 'high',
    defaultNamePattern: 'qa-{n}',
    focusArea: 'Verification and regression testing',
    contextSummary: 'Owns test plans, verification evidence, and regression reproduction context.',
    behaviorSummary: 'Expands coverage and escalates ambiguous requirements instead of guessing.',
    capabilities: ['testing', 'verification'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Own verification strategy, regression reproduction, and evidence gathering for shipped changes.',
    behavioralContract: {
      communication: ['Report verification scope and remaining risk clearly.'],
      execution: ['Add regression coverage for reproduced bugs.'],
      escalation: ['Escalate when requirements are too ambiguous to verify credibly.'],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 4,
      requiresLeadTool: null,
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'claude-reviewer',
    name: 'Claude Reviewer',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'claude',
    model: 'claude-opus-4-6',
    defaultNamePattern: 'reviewer-{n}',
    focusArea: 'Change review',
    contextSummary: 'Reviews for correctness, regression risk, and missing coverage.',
    behaviorSummary: 'Finds concrete risks and avoids speculative redesign requests.',
    capabilities: ['review', 'security', 'risk-analysis', 'testing'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Review changes for correctness, regressions, security risk, and missing tests. Prioritize actionable findings.',
    behavioralContract: {
      communication: [
        'Confirm review scope before starting.',
        'Report findings ordered by severity with file references.',
      ],
      execution: [
        'Focus on behavior/regression risk over style nitpicks.',
        'Highlight residual risks when no critical findings exist.',
      ],
      escalation: ['Escalate high-risk defects immediately.'],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 6,
      requiresLeadTool: 'claude',
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'gemini-ui-specialist',
    name: 'Gemini UI Specialist',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'gemini',
    model: 'gemini-3.1-pro',
    defaultNamePattern: 'ui-{n}',
    focusArea: 'Frontend presentation and visual polish',
    contextSummary: 'Owns layout, interaction polish, and design-system coherence across UI changes.',
    behaviorSummary: 'Pushes the visual/design bar while escalating unclear product intent.',
    capabilities: ['ui', 'design'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Own UI presentation, layout clarity, and polish for assigned frontend surfaces.',
    behavioralContract: {
      communication: ['Share visual direction and constraints clearly.'],
      execution: ['Preserve existing design language unless the task calls for redesign.'],
      escalation: ['Escalate unclear product intent or conflicting visual requirements.'],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 4,
      requiresLeadTool: null,
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'gemini-orchestrator',
    name: 'Gemini Orchestrator',
    version: '1.0.0',
    kind: 'lead',
    cliTool: 'gemini',
    model: 'gemini-3.1-pro',
    defaultNamePattern: 'lead-{project}',
    focusArea: 'Research-guided orchestration',
    contextSummary: 'Keeps cross-functional context, research findings, and delivery sequencing aligned across the team.',
    behaviorSummary: 'Routes work deliberately and escalates ambiguous direction instead of inventing implementation detail.',
    capabilities: ['coordination', 'research', 'synthesis'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Coordinate research-heavy delivery, keep context summaries current, and escalate ambiguity before it spreads across the team.',
    behavioralContract: {
      communication: [
        'Clarify intent and desired evidence before delegating.',
        'Summarize findings and decisions as the team advances.',
      ],
      execution: [
        'Keep research, implementation, and review lanes aligned.',
        'Avoid speculative implementation ownership.',
      ],
      escalation: [
        'Escalate unclear product direction or conflicting evidence immediately.',
      ],
    },
    constraints: {
      minInstances: 1,
      maxInstances: 1,
      requiresLeadTool: null,
      allowedProjectBinding: 'lead_project',
    },
  },
  {
    roleId: 'custom-doc-writer',
    name: 'Documentation Writer',
    version: '0.2.0',
    kind: 'agent',
    cliTool: 'gemini',
    model: 'gemini-3.1-pro',
    defaultNamePattern: 'docs-{n}',
    focusArea: 'Documentation systems',
    contextSummary: 'Maintains operational docs and architecture-facing explanations.',
    behaviorSummary: 'Clarifies shipped behavior without assuming implementation ownership.',
    capabilities: ['documentation', 'research'],
    builtIn: false,
    readOnly: false,
    instructions:
      'Produce concise documentation updates and cross-link architecture references.',
    behavioralContract: {
      communication: ['Share draft structure early for review.'],
      execution: ['Keep docs consistent with shipped behavior.'],
      escalation: ['Flag stale or conflicting docs as risks.'],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 4,
      requiresLeadTool: null,
      allowedProjectBinding: 'any',
    },
  },
]

export function mockRoleExportResult(roleId, targetFormat) {
  const role = MOCK_ROLE_TEMPLATES.find((entry) => entry.roleId === roleId)
  if (!role) {
    throw new Error(`Role not found: ${roleId}`)
  }

  return {
    targetFormat,
    fileContent: `# ${role.name}\n\n${role.instructions ?? ''}\n`,
    lossyFields: ['capabilities', 'constraints'],
  }
}

export const MOCK_TEAM_PRESETS = [
  {
    presetId: 'pair',
    name: 'Pair',
    description: 'One lead and one quick-delivery developer for the smallest scoped build-and-review loop.',
    version: '3.0.0',
    leadRoleId: 'v3-lead-claude',
    builtIn: true,
    readOnly: true,
    agentSlots: [
      {
        roleId: 'quick-dev-codex',
        count: 1,
        projectBinding: 'lead_project',
        overrides: { namePattern: 'quick-dev' },
      },
    ],
    defaults: { teamNamePattern: '{project}-team', tmuxLayout: 'tiled' },
  },
  {
    presetId: 'dev-team',
    name: 'Dev Team',
    description: 'One lead and two vertical-slice developers for parallel product-visible implementation with shared review gates.',
    version: '3.0.0',
    leadRoleId: 'v3-lead-claude',
    builtIn: true,
    readOnly: true,
    agentSlots: [
      { roleId: 'v3-developer-codex', count: 2, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-team', tmuxLayout: 'tiled' },
  },
  {
    presetId: 'full-team',
    name: 'Full Team',
    description: 'One lead, one architect, and two developers for structural guidance, implementation throughput, and stronger readiness checks.',
    version: '3.0.0',
    leadRoleId: 'v3-lead-claude',
    builtIn: true,
    readOnly: true,
    agentSlots: [
      {
        roleId: 'v3-architect-codex',
        count: 1,
        projectBinding: 'lead_project',
        overrides: { namePattern: 'architect' },
      },
      { roleId: 'v3-developer-codex', count: 2, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-team', tmuxLayout: 'tiled' },
  },
  {
    presetId: 'research-team',
    name: 'Research Team',
    description: 'One lead, one researcher, and one developer for evidence gathering paired with implementation and decision-ready handoff.',
    version: '3.0.0',
    leadRoleId: 'v3-lead-claude',
    builtIn: true,
    readOnly: true,
    agentSlots: [
      {
        roleId: 'claude-researcher',
        count: 1,
        projectBinding: 'lead_project',
        overrides: { namePattern: 'researcher' },
      },
      { roleId: 'v3-developer-codex', count: 1, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-team', tmuxLayout: 'tiled' },
  },
  {
    presetId: 'docs-sprint',
    name: 'Docs Sprint Team',
    description: 'Lead plus one documentation-focused agent.',
    version: '0.2.0',
    leadRoleId: 'claude-orchestrator',
    builtIn: false,
    readOnly: false,
    agentSlots: [
      { roleId: 'custom-doc-writer', count: 1, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-docs', tmuxLayout: 'even-horizontal' },
  },
]

export const MOCK_TEMPLATE_STORAGE_STATUS = {
  mode: 'git',
  repoInitialized: true,
  dirty: true,
  pendingActions: [],
  lastCommit: Math.floor(Date.now() / 1000) - 3600,
}

export const MOCK_TEMPLATE_HISTORY = [
  {
    commitId: 'f3b6c841d1f84b7e1a2c9018899f1f37f71aa001',
    shortId: 'f3b6c841',
    message: 'templates: tune claude reviewer rubric',
    author: 'taurhaus-dev-1',
    timestamp: Math.floor(Date.now() / 1000) - 600,
    changedPaths: ['roles/claude-reviewer.yaml'],
  },
  {
    commitId: 'de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322',
    shortId: 'de11ab00',
    message: 'templates: add docs sprint preset',
    author: 'taurhaus-dev-2',
    timestamp: Math.floor(Date.now() / 1000) - 2200,
    changedPaths: ['presets/docs-sprint.yaml', '_meta/state.json'],
  },
  {
    commitId: '9cc7fb70f1bf8da99ef8d8e50b179e744f5e6f10',
    shortId: '9cc7fb70',
    message: 'templates: introduce codex developer role',
    author: 'team-lead',
    timestamp: Math.floor(Date.now() / 1000) - 4800,
    changedPaths: ['roles/codex-developer.yaml'],
  },
]

export const MOCK_TEMPLATE_DIFFS = {
  f3b6c841d1f84b7e1a2c9018899f1f37f71aa001: {
    commitId: 'f3b6c841d1f84b7e1a2c9018899f1f37f71aa001',
    files: [
      {
        path: 'roles/claude-reviewer.yaml',
        status: 'modified',
        hunks: [
          {
            old_start: 12,
            old_lines: 3,
            new_start: 12,
            new_lines: 4,
            lines: [
              { origin: ' ', old_lineno: 12, new_lineno: 12, content: 'behavioral_contract:' },
              {
                origin: '-',
                old_lineno: 13,
                new_lineno: null,
                content: '  execution: [focus on correctness]',
              },
              {
                origin: '+',
                old_lineno: null,
                new_lineno: 13,
                content: '  execution: [focus on correctness and regression risk]',
              },
              {
                origin: '+',
                old_lineno: null,
                new_lineno: 14,
                content: '  escalation: [raise high-risk findings immediately]',
              },
            ],
          },
        ],
      },
    ],
    stats: { filesChanged: 1, insertions: 2, deletions: 1 },
  },
  de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322: {
    commitId: 'de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322',
    files: [
      {
        path: 'presets/docs-sprint.yaml',
        status: 'added',
        hunks: [
          {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: 5,
            lines: [
              { origin: '+', old_lineno: null, new_lineno: 1, content: 'preset_id: docs-sprint' },
              { origin: '+', old_lineno: null, new_lineno: 2, content: 'lead_role_id: claude-orchestrator' },
              { origin: '+', old_lineno: null, new_lineno: 3, content: 'agent_slots:' },
              { origin: '+', old_lineno: null, new_lineno: 4, content: '  - role_id: custom-doc-writer' },
              { origin: '+', old_lineno: null, new_lineno: 5, content: '    count: 1' },
            ],
          },
        ],
      },
    ],
    stats: { filesChanged: 1, insertions: 5, deletions: 0 },
  },
}

export function roleTemplateSummary(template) {
  return {
    roleId: template.roleId,
    name: template.name,
    kind: template.kind,
    cliTool: template.cliTool,
    model: template.model,
    reasoningEffort: template.reasoningEffort ?? null,
    focusArea: template.focusArea ?? '',
    contextSummary: template.contextSummary ?? '',
    behaviorSummary: template.behaviorSummary ?? '',
    builtIn: Boolean(template.builtIn),
    readOnly: Boolean(template.readOnly),
  }
}

export function teamPresetSummary(preset) {
  const referencedRoles = [preset.leadRoleId, ...(preset.agentSlots ?? []).map((slot) => slot.roleId)]
    .map((roleId) => MOCK_ROLE_TEMPLATES.find((role) => role.roleId === roleId))
    .filter(Boolean)

  const tools = [...new Set(referencedRoles.map((role) => role.cliTool))]

  return {
    presetId: preset.presetId,
    name: preset.name,
    description: preset.description,
    leadRoleId: preset.leadRoleId,
    roleCount: preset.agentSlots?.length ?? 0,
    agentCount: (preset.agentSlots ?? []).reduce((total, slot) => total + (slot.count ?? 0), 0),
    tools,
    builtIn: Boolean(preset.builtIn),
    readOnly: Boolean(preset.readOnly),
  }
}
