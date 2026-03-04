<script>
  import { composeTeam, listRoleTemplates } from '../ipc.js'
  import { getToolIcon, getToolName } from '../toolLogos.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    projectPath = '',
    projectName = '',
    availableTools = ['claude', 'codex', 'gemini'],
    initialPreset = null,
    onApply = () => {},
    onSavePreset = () => {},
    onClose = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const sectionTone = $derived(
    dark ? 'border-zinc-700/70 bg-zinc-900/60' : 'border-zinc-200 bg-white'
  )
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
      : 'border-zinc-300 bg-white text-zinc-900 placeholder:text-zinc-500'
  )
  const neutralButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-200 hover:bg-zinc-800'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )

  let loadingRoles = $state(false)
  let loadError = $state('')
  let roleTemplates = $state([])
  let leadRoleId = $state('')
  let agentCounts = $state({})
  let composedResult = $state({
    roster: [],
    warnings: [],
    validationErrors: [],
  })
  let editedRoster = $state([])
  let composeError = $state('')
  let showSaveDialog = $state(false)
  let saveName = $state('')
  let saveDescription = $state('')
  let saveError = $state('')
  let saveNotice = $state('')
  let appliedPresetFingerprint = $state('')
  let composeSequence = 0

  const roleOptions = $derived.by(() =>
    roleTemplates.map((entry) => normalizeRoleTemplate(entry)).filter((entry) => entry.roleId)
  )

  const leadRoles = $derived.by(() => roleOptions.filter((role) => role.kind === 'lead'))
  const agentRoles = $derived.by(() => roleOptions.filter((role) => role.kind === 'agent'))

  const selectedAgentSlots = $derived.by(() => {
    const slots = []
    for (const role of agentRoles) {
      const count = Number(agentCounts[role.roleId] ?? 0)
      if (count > 0) {
        slots.push({
          roleId: role.roleId,
          count,
          projectBinding: 'lead_project',
          overrides: null,
        })
      }
    }
    return slots
  })

  const resolvedProjectName = $derived.by(() => {
    if (String(projectName || '').trim()) return String(projectName).trim()
    const parts = String(projectPath || '')
      .split(/[\\/]+/)
      .filter(Boolean)
    return parts.at(-1) ?? 'project'
  })

  const validation = $derived.by(() => {
    const errors = []
    const warnings = []

    if (composeError) errors.push(composeError)
    for (const message of composedResult.validationErrors ?? []) errors.push(String(message))
    for (const message of composedResult.warnings ?? []) warnings.push(String(message))

    const leadCount = editedRoster.filter((member) => member.roleKind === 'lead').length
    if (leadCount !== 1) {
      errors.push(`Lead check failed: expected 1 lead, found ${leadCount}.`)
    }

    const duplicateNames = findDuplicateNames(editedRoster)
    if (duplicateNames.length > 0) {
      errors.push(`Name collisions: ${duplicateNames.join(', ')}.`)
    }

    const available = new Set((availableTools ?? []).map((tool) => String(tool).toLowerCase()))
    const unavailableTools = Array.from(
      new Set(
        editedRoster
          .map((member) => String(member.cliTool || '').toLowerCase())
          .filter(Boolean)
          .filter((tool) => available.size > 0 && !available.has(tool))
      )
    )
    if (unavailableTools.length > 0) {
      warnings.push(`Tool availability warning: ${unavailableTools.join(', ')} unavailable.`)
    }

    return {
      leadCount,
      leadOk: leadCount === 1,
      duplicateNames,
      unavailableTools,
      errors: uniqueMessages(errors),
      warnings: uniqueMessages(warnings),
    }
  })

  const canApply = $derived(validation.errors.length === 0 && editedRoster.length > 0)

  function uniqueMessages(messages) {
    const seen = new Set()
    const output = []
    for (const message of messages.map((entry) => String(entry || '').trim()).filter(Boolean)) {
      if (seen.has(message)) continue
      seen.add(message)
      output.push(message)
    }
    return output
  }

  function normalizeRoleTemplate(value) {
    return {
      roleId: value?.roleId ?? value?.role_id ?? '',
      name: value?.name ?? '',
      kind: String(value?.kind ?? 'agent').toLowerCase(),
      cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
      model: value?.model ?? '',
      capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
    }
  }

  function normalizePreset(value) {
    if (!value || typeof value !== 'object') return null
    const agentSlotsRaw = value?.agentSlots ?? value?.agent_slots ?? []
    return {
      presetId: value?.presetId ?? value?.preset_id ?? '',
      name: value?.name ?? '',
      description: value?.description ?? '',
      leadRoleId: value?.leadRoleId ?? value?.lead_role_id ?? '',
      agentSlots: Array.isArray(agentSlotsRaw)
        ? agentSlotsRaw.map((slot) => ({
            roleId: slot?.roleId ?? slot?.role_id ?? '',
            count: Number(slot?.count ?? 0),
          }))
        : [],
    }
  }

  function normalizeResolvedMember(value) {
    return {
      name: value?.name ?? '',
      roleId: value?.roleId ?? value?.role_id ?? '',
      roleKind: String(value?.roleKind ?? value?.role_kind ?? 'agent').toLowerCase(),
      cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
      model: value?.model ?? '',
      instructions: value?.instructions ?? '',
      projectBinding: value?.projectBinding ?? value?.project_binding ?? 'lead_project',
      projectId: value?.projectId ?? value?.project_id ?? '',
    }
  }

  function normalizeCompositionResult(value) {
    const validationErrors = value?.validationErrors ?? value?.validation_errors ?? []
    return {
      roster: Array.isArray(value?.roster) ? value.roster.map(normalizeResolvedMember) : [],
      warnings: Array.isArray(value?.warnings) ? value.warnings : [],
      validationErrors: Array.isArray(validationErrors) ? validationErrors : [],
    }
  }

  function toolIcon(tool) {
    return getToolIcon(tool)
  }

  function toolLabel(tool) {
    return getToolName(tool)
  }

  function roleCount(roleId) {
    return Number(agentCounts[roleId] ?? 0)
  }

  function setRoleCount(roleId, count) {
    const normalized = Math.max(0, Number(count) || 0)
    agentCounts = {
      ...agentCounts,
      [roleId]: normalized,
    }
  }

  function increaseRoleCount(roleId) {
    setRoleCount(roleId, roleCount(roleId) + 1)
  }

  function decreaseRoleCount(roleId) {
    setRoleCount(roleId, roleCount(roleId) - 1)
  }

  function presetFingerprint(preset) {
    if (!preset) return ''
    const slots = (preset.agentSlots ?? [])
      .map((slot) => `${slot.roleId}:${slot.count}`)
      .sort((left, right) => left.localeCompare(right))
      .join('|')
    return `${preset.presetId}|${preset.leadRoleId}|${slots}`
  }

  function hydrateFromPreset(preset) {
    leadRoleId = preset?.leadRoleId ?? ''
    const nextCounts = {}
    for (const slot of preset?.agentSlots ?? []) {
      if (!slot?.roleId) continue
      nextCounts[slot.roleId] = Math.max(0, Number(slot.count ?? 0))
    }
    agentCounts = nextCounts
  }

  function buildComposeRequest() {
    if (!leadRoleId) return null
    return {
      leadRoleId,
      agentSlots: selectedAgentSlots,
      projectName: resolvedProjectName,
    }
  }

  function mergeEditedRoster(nextRoster) {
    const existing = new Map(editedRoster.map((entry) => [entry._key, entry]))
    const counters = new Map()
    editedRoster = nextRoster.map((member) => {
      const roleKey = `${member.roleKind}:${member.roleId}`
      const count = (counters.get(roleKey) ?? 0) + 1
      counters.set(roleKey, count)
      const key = `${roleKey}:${count}`
      const prior = existing.get(key)
      return {
        ...member,
        _key: key,
        name: prior?.name ?? member.name,
        cliTool: prior?.cliTool ?? member.cliTool,
        model: prior?.model ?? member.model,
        instructions: prior?.instructions ?? member.instructions,
      }
    })
  }

  function findDuplicateNames(roster) {
    const counts = new Map()
    for (const [index, member] of roster.entries()) {
      const name = normalizedMemberName(member, index).toLowerCase()
      counts.set(name, (counts.get(name) ?? 0) + 1)
    }
    return Array.from(counts.entries())
      .filter(([, count]) => count > 1)
      .map(([name]) => name)
  }

  function normalizedMemberName(member, index) {
    const explicitName = String(member?.name ?? '').trim()
    if (explicitName) return explicitName
    const base = member?.roleKind === 'lead' ? 'lead' : 'agent'
    return `${base}-${index + 1}`
  }

  function updateRosterMember(index, patch) {
    editedRoster = editedRoster.map((member, current) =>
      current === index ? { ...member, ...patch } : member
    )
  }

  async function loadRoles() {
    loadingRoles = true
    loadError = ''
    try {
      roleTemplates = await listRoleTemplates()
      if (!leadRoleId) {
        leadRoleId = leadRoles[0]?.roleId ?? ''
      }
    } catch (error) {
      roleTemplates = []
      loadError = error?.message || 'Failed to load role templates.'
    } finally {
      loadingRoles = false
    }
  }

  async function composeCurrentSelection(request) {
    const sequence = ++composeSequence
    composeError = ''
    try {
      const result = await composeTeam(request)
      if (sequence !== composeSequence) return
      composedResult = normalizeCompositionResult(result)
      mergeEditedRoster(composedResult.roster)
    } catch (error) {
      if (sequence !== composeSequence) return
      composedResult = {
        roster: [],
        warnings: [],
        validationErrors: [],
      }
      editedRoster = []
      composeError = error?.message || 'Failed to compose roster.'
    }
  }

  function openSavePresetDialog() {
    showSaveDialog = true
    saveError = ''
    saveNotice = ''
    if (!saveName.trim()) {
      saveName = `${resolvedProjectName}-custom`
    }
  }

  function closeSavePresetDialog() {
    showSaveDialog = false
    saveError = ''
  }

  function slugifyPresetId(value) {
    return String(value || '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
  }

  async function savePreset() {
    saveError = ''
    const trimmedName = saveName.trim()
    if (!trimmedName) {
      saveError = 'Preset name is required.'
      return
    }

    const payload = {
      presetId: slugifyPresetId(trimmedName),
      name: trimmedName,
      description: saveDescription.trim(),
      leadRoleId,
      agentSlots: selectedAgentSlots,
    }

    try {
      await Promise.resolve(onSavePreset(payload))
      showSaveDialog = false
      saveNotice = `Saved preset "${trimmedName}".`
    } catch (error) {
      saveError = error?.message || 'Failed to save preset.'
    }
  }

  function applyComposition() {
    if (!canApply) return
    const leadMember = editedRoster.find((member) => member.roleKind === 'lead')
    if (!leadMember) return

    const agents = editedRoster.filter((member) => member.roleKind !== 'lead')
    const payload = {
      leadMode: 'launch_new',
      leadRoleId,
      agentSlots: selectedAgentSlots,
      lead: {
        name: normalizedMemberName(leadMember, 0),
        cliTool: leadMember.cliTool,
        model: leadMember.model,
        projectId: leadMember.projectId || projectPath,
        description: leadMember.roleId || null,
      },
      agents: agents.map((member, index) => ({
        name: normalizedMemberName(member, index + 1),
        cliTool: member.cliTool,
        model: member.model,
        projectId: member.projectId || projectPath,
        description: member.roleId || null,
      })),
      roster: editedRoster.map((member, index) => ({
        name: normalizedMemberName(member, index),
        roleId: member.roleId,
        roleKind: member.roleKind,
        cliTool: member.cliTool,
        model: member.model,
        instructions: member.instructions,
        projectBinding: member.projectBinding,
        projectId: member.projectId || projectPath,
      })),
      validation: {
        errors: validation.errors,
        warnings: validation.warnings,
      },
    }
    onApply(payload)
  }

  $effect(() => {
    void loadRoles()
  })

  $effect(() => {
    const normalized = normalizePreset(initialPreset)
    const fingerprint = presetFingerprint(normalized)
    if (!fingerprint || fingerprint === appliedPresetFingerprint) return
    appliedPresetFingerprint = fingerprint
    hydrateFromPreset(normalized)
  })

  $effect(() => {
    const request = buildComposeRequest()
    if (!request) {
      composedResult = {
        roster: [],
        warnings: [],
        validationErrors: [],
      }
      editedRoster = []
      return
    }
    void composeCurrentSelection(request)
  })
</script>

<section class="space-y-3 rounded-lg border p-3 {sectionTone}" data-testid="team-composer">
  <header class="flex items-start justify-between gap-2">
    <div>
      <h2 class="text-sm font-semibold {t.textPrimary}">Team Composer</h2>
      <p class="text-[11px] {t.textMuted}">
        Pick a lead, choose agent counts, review the generated roster, then apply.
      </p>
    </div>
    <button
      class="rounded-md border px-2 py-1 text-[11px] {neutralButton}"
      onclick={() => onClose()}
      data-testid="composer-close"
    >
      Close
    </button>
  </header>

  {#if loadError}
    <p class="rounded-md border border-danger-500/40 bg-danger-500/10 px-2 py-1 text-xs text-danger-400">
      {loadError}
    </p>
  {/if}

  <section class="rounded-md border p-2 {sectionTone}" data-testid="lead-role-picker">
    <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Lead Role Picker</h3>
    <label class="mt-2 flex flex-col gap-1">
      <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Lead role</span>
      <select
        class="h-8 rounded-md border px-2 text-xs {inputTone}"
        value={leadRoleId}
        onchange={(event) => {
          leadRoleId = event.currentTarget.value
        }}
        disabled={loadingRoles}
        data-testid="composer-lead-select"
      >
        <option value="">Select lead role</option>
        {#each leadRoles as role}
          <option value={role.roleId}>{role.name} ({role.roleId})</option>
        {/each}
      </select>
    </label>
  </section>

  <section class="rounded-md border p-2 {sectionTone}" data-testid="agent-role-selector">
    <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Agent Role Selector</h3>
    {#if agentRoles.length === 0}
      <p class="mt-2 text-xs {t.textMuted}">No agent templates available.</p>
    {:else}
      <div class="mt-2 space-y-2">
        {#each agentRoles as role}
          <article
            class="flex items-center justify-between gap-2 rounded-md border p-2 {dark ? 'border-zinc-700/60 bg-zinc-900/40' : 'border-zinc-200 bg-zinc-50'}"
            data-testid={`agent-stepper-${role.roleId}`}
          >
            <div class="min-w-0">
              <p class="text-[12px] font-medium {t.textPrimary}">
                {role.name}
              </p>
              <p class="text-[10px] {t.textMuted}">
                {role.roleId} | {toolLabel(role.cliTool)} | {role.model}
              </p>
            </div>
            <div class="flex items-center gap-1">
              <button
                class="h-6 w-6 rounded border text-xs {neutralButton}"
                onclick={() => decreaseRoleCount(role.roleId)}
                data-testid={`agent-decrease-${role.roleId}`}
              >
                -
              </button>
              <span
                class="inline-flex h-6 min-w-8 items-center justify-center rounded border px-1 text-xs {inputTone}"
                data-testid={`agent-count-${role.roleId}`}
              >
                {roleCount(role.roleId)}
              </span>
              <button
                class="h-6 w-6 rounded border text-xs {neutralButton}"
                onclick={() => increaseRoleCount(role.roleId)}
                data-testid={`agent-increase-${role.roleId}`}
              >
                +
              </button>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <section class="rounded-md border p-2 {sectionTone}" data-testid="roster-review">
    <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Roster Review</h3>
    {#if editedRoster.length === 0}
      <p class="mt-2 text-xs {t.textMuted}">
        Select a lead role to generate the roster.
      </p>
    {:else}
      <div class="mt-2 space-y-2">
        {#each editedRoster as member, index}
          <article
            class="rounded-md border p-2 {dark ? 'border-zinc-700/60 bg-zinc-900/40' : 'border-zinc-200 bg-zinc-50'}"
            data-testid={`composer-roster-card-${index}`}
          >
            <div class="flex items-center justify-between gap-2">
              <p class="text-[12px] font-medium {t.textPrimary}">
                {member.roleId} · {member.roleKind}
              </p>
              <span class="text-[10px] {t.textMuted}">
                {index === 0 ? 'Lead lane' : `Agent lane ${index}`}
              </span>
            </div>

            <div class="mt-2 grid grid-cols-1 gap-2 md:grid-cols-2">
              <label class="flex flex-col gap-1">
                <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Name</span>
                <input
                  class="h-8 rounded-md border px-2 text-xs {inputTone}"
                  value={member.name}
                  oninput={(event) => {
                    updateRosterMember(index, { name: event.currentTarget.value })
                  }}
                  data-testid={`composer-roster-name-${index}`}
                />
              </label>
              <label class="flex flex-col gap-1">
                <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Tool</span>
                <select
                  class="h-8 rounded-md border px-2 text-xs {inputTone}"
                  value={member.cliTool}
                  onchange={(event) => {
                    updateRosterMember(index, { cliTool: event.currentTarget.value })
                  }}
                  data-testid={`composer-roster-tool-${index}`}
                >
                  <option value="claude">Claude</option>
                  <option value="codex">Codex</option>
                  <option value="gemini">Gemini</option>
                </select>
              </label>
              <label class="flex flex-col gap-1">
                <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Model</span>
                <input
                  class="h-8 rounded-md border px-2 text-xs {inputTone}"
                  value={member.model}
                  oninput={(event) => {
                    updateRosterMember(index, { model: event.currentTarget.value })
                  }}
                  data-testid={`composer-roster-model-${index}`}
                />
              </label>
              <label class="flex flex-col gap-1">
                <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Project ID</span>
                <input
                  class="h-8 rounded-md border px-2 text-xs {inputTone}"
                  value={member.projectId || projectPath}
                  oninput={(event) => {
                    updateRosterMember(index, { projectId: event.currentTarget.value })
                  }}
                  data-testid={`composer-roster-project-${index}`}
                />
              </label>
            </div>

            <label class="mt-2 flex flex-col gap-1">
              <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Instructions</span>
              <textarea
                class="min-h-16 rounded-md border px-2 py-1.5 text-xs {inputTone}"
                value={member.instructions}
                oninput={(event) => {
                  updateRosterMember(index, { instructions: event.currentTarget.value })
                }}
                data-testid={`composer-roster-instructions-${index}`}
              ></textarea>
            </label>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <section class="rounded-md border p-2 {sectionTone}" data-testid="composition-validator">
    <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Composition Validator</h3>
    <div class="mt-2 grid grid-cols-1 gap-1 text-[11px] md:grid-cols-3">
      <p data-testid="composer-validation-lead">
        Lead check:
        <span class={validation.leadOk ? 'text-success-500' : 'text-danger-500'}>
          {validation.leadOk ? 'OK' : `Expected 1, found ${validation.leadCount}`}
        </span>
      </p>
      <p data-testid="composer-validation-tools">
        Tool availability:
        <span class={validation.unavailableTools.length === 0 ? 'text-success-500' : 'text-warning-500'}>
          {validation.unavailableTools.length === 0
            ? 'OK'
            : `Unavailable ${validation.unavailableTools.join(', ')}`}
        </span>
      </p>
      <p data-testid="composer-validation-names">
        Name collisions:
        <span class={validation.duplicateNames.length === 0 ? 'text-success-500' : 'text-danger-500'}>
          {validation.duplicateNames.length === 0
            ? 'None'
            : validation.duplicateNames.join(', ')}
        </span>
      </p>
    </div>

    {#if validation.errors.length > 0}
      <ul class="mt-2 space-y-1 rounded-md border border-danger-500/40 bg-danger-500/10 p-2 text-xs text-danger-400" data-testid="composer-validation-errors">
        {#each validation.errors as error}
          <li>{error}</li>
        {/each}
      </ul>
    {/if}

    {#if validation.warnings.length > 0}
      <ul class="mt-2 space-y-1 rounded-md border border-warning-500/40 bg-warning-500/10 p-2 text-xs text-warning-400" data-testid="composer-validation-warnings">
        {#each validation.warnings as warning}
          <li>{warning}</li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="rounded-md border p-2 {sectionTone}" data-testid="save-as-preset">
    <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Save As Preset</h3>
    <div class="mt-2 flex items-center justify-between gap-2">
      <p class="text-xs {t.textMuted}">
        Optional: persist this composition as a user preset.
      </p>
      <button
        class="rounded-md border px-2 py-1 text-[11px] {neutralButton}"
        onclick={openSavePresetDialog}
        data-testid="composer-save-open"
      >
        Save as Preset
      </button>
    </div>

    {#if saveNotice}
      <p class="mt-2 text-xs text-success-500">{saveNotice}</p>
    {/if}
  </section>

  {#if showSaveDialog}
    <div class="rounded-md border p-2 {sectionTone}" data-testid="composer-save-dialog">
      <div class="grid grid-cols-1 gap-2 md:grid-cols-2">
        <label class="flex flex-col gap-1">
          <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Preset name</span>
          <input
            class="h-8 rounded-md border px-2 text-xs {inputTone}"
            value={saveName}
            oninput={(event) => {
              saveName = event.currentTarget.value
            }}
            data-testid="composer-save-name"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Description</span>
          <input
            class="h-8 rounded-md border px-2 text-xs {inputTone}"
            value={saveDescription}
            oninput={(event) => {
              saveDescription = event.currentTarget.value
            }}
            data-testid="composer-save-description"
          />
        </label>
      </div>

      {#if saveError}
        <p class="mt-2 text-xs text-danger-500">{saveError}</p>
      {/if}

      <div class="mt-2 flex items-center justify-end gap-1">
        <button
          class="rounded-md border px-2 py-1 text-[11px] {neutralButton}"
          onclick={closeSavePresetDialog}
          data-testid="composer-save-cancel"
        >
          Cancel
        </button>
        <button
          class="rounded-md bg-brand-600 px-2 py-1 text-[11px] font-medium text-white hover:bg-brand-700"
          onclick={savePreset}
          data-testid="composer-save-submit"
        >
          Save
        </button>
      </div>
    </div>
  {/if}

  <footer class="flex items-center justify-between gap-2">
    <div class="flex items-center gap-1 text-[11px] {t.textMuted}">
      <svg class="h-3 w-3" viewBox={toolIcon('claude').viewBox} fill="currentColor" aria-hidden="true">
        <path d={toolIcon('claude').path}></path>
      </svg>
      <span>Composition uses template defaults then inline edits.</span>
    </div>
    <button
      class="rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
      onclick={applyComposition}
      disabled={!canApply}
      data-testid="composer-apply"
    >
      Apply to Mesh Init
    </button>
  </footer>
</section>
