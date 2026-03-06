<script>
  import { getToolIcon, getToolName } from '../toolLogos.js'
  import {
    isCustomRole,
    roleKindBadgeTone,
  } from './templateBrowserUtils.js'

  let {
    dark = false,
    t,
    cardTone = '',
    actionSecondary = '',
    toneMuted = '',
    detailKind = '',
    detailLoading = false,
    selectedRole = null,
    filteredRoleTemplates = [],
    hasCustomRoles = false,
    onSelectRole = () => {},
    onResetDetail = () => {},
    onOpenCreateRoleEditor = () => {},
    onInspectRole = () => {},
    onOpenEditRoleEditor = () => {},
    onRequestRoleDelete = () => {},
  } = $props()
</script>

{#if detailKind === 'role'}
  <section class="rounded-xl border p-4 space-y-4 animate-in fade-in slide-in-from-left-2 duration-200 {cardTone}" data-testid="template-role-detail">
    <button
      class="inline-flex items-center gap-1.5 h-8 px-2.5 rounded-lg text-[11px] font-bold uppercase tracking-wide {actionSecondary}"
      onclick={onResetDetail}
      data-testid="template-role-back"
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
      Back
    </button>

    {#if detailLoading}
      <p class="text-xs text-center py-4 {toneMuted}">Loading role details...</p>
    {:else if selectedRole}
      <div class="space-y-2">
        <div class="flex items-start justify-between">
          <h3 class="text-base font-bold {t.textPrimary}">
            {selectedRole.name}
          </h3>
          <span class="rounded-full px-2 py-0.5 text-[10px] font-bold {roleKindBadgeTone(selectedRole.kind, dark)}">{selectedRole.kind}</span>
        </div>
        <p class="text-[10px] font-mono {toneMuted}">{selectedRole.roleId}</p>
      </div>

      <div class="flex flex-wrap items-center gap-1.5">
        <span
          class="inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[10px] font-bold {dark ? 'border-zinc-700 text-zinc-300 bg-zinc-900/80' : 'border-zinc-200 text-zinc-700 bg-zinc-50'}"
          data-testid={`role-tool-badge-${selectedRole.roleId}`}
        >
          <svg class="h-3 w-3 shrink-0" viewBox={getToolIcon(selectedRole.cliTool).viewBox} fill="currentColor" aria-hidden="true">
            <path d={getToolIcon(selectedRole.cliTool).path}></path>
          </svg>
          <span class="uppercase tracking-tighter opacity-80">{getToolName(selectedRole.cliTool)}</span>
        </span>
        <span
          class="inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-bold {dark ? 'border-zinc-700 text-zinc-300 bg-zinc-900/80' : 'border-zinc-200 text-zinc-700 bg-zinc-50'}"
          data-testid={`role-model-badge-${selectedRole.roleId}`}
        >
          {selectedRole.model || 'unspecified'}
        </span>
      </div>

      <div class="grid gap-2">
        <section class="rounded-lg border p-3 {dark ? 'border-white/8 bg-white/[0.03]' : 'border-zinc-200 bg-white/70'}">
          <p class="text-[10px] font-bold uppercase tracking-wide {toneMuted}">Focus area</p>
          <p class="mt-1 text-xs font-semibold {t.textPrimary}">
            {selectedRole.focusArea || 'No focus area defined.'}
          </p>
        </section>

        <section class="rounded-lg border p-3 {dark ? 'border-white/8 bg-white/[0.03]' : 'border-zinc-200 bg-white/70'}">
          <p class="text-[10px] font-bold uppercase tracking-wide {toneMuted}">Context summary</p>
          <p class="mt-1 text-xs leading-relaxed {t.textSecondary}">
            {selectedRole.contextSummary || 'No context summary available.'}
          </p>
        </section>

        <section class="rounded-lg border p-3 {dark ? 'border-white/8 bg-white/[0.03]' : 'border-zinc-200 bg-white/70'}">
          <p class="text-[10px] font-bold uppercase tracking-wide {toneMuted}">Behavioral boundary</p>
          <p class="mt-1 text-xs leading-relaxed {t.textSecondary}">
            {selectedRole.behaviorSummary || 'No behavioral boundary available.'}
          </p>
        </section>
      </div>

      <details class="rounded-lg border p-3 {dark ? 'border-white/8 bg-black/10' : 'border-zinc-200 bg-zinc-50/80'}">
        <summary class="cursor-pointer text-[10px] font-bold uppercase tracking-wide {toneMuted}">
          Raw instructions
        </summary>
        <p class="mt-2 text-xs leading-relaxed {t.textSecondary}">
          {selectedRole.instructions || 'No role instructions available.'}
        </p>
      </details>

      <button
        class="w-full h-10 rounded-lg bg-brand-600 px-4 py-1 text-xs font-bold text-white hover:bg-brand-500 shadow-lg shadow-brand-500/20 active:scale-95 transition-all"
        onclick={() => onSelectRole(selectedRole)}
        data-testid={`role-select-${selectedRole.roleId}`}
      >
        Use this Role
      </button>
    {/if}
  </section>
{:else if filteredRoleTemplates.length === 0}
  <div class="flex flex-col items-center justify-center py-12 border-2 border-dashed rounded-xl {dark ? 'border-zinc-800' : 'border-zinc-200'}">
    <p class="text-xs {t.textMuted}">
      No role templates match the current filter.
    </p>
  </div>
{:else}
  <div class="flex items-center justify-between px-1">
    <p class="text-[10px] font-bold uppercase tracking-wider {t.textMuted}">Role Templates</p>
    <button
      class="h-8 px-3 rounded-lg text-[11px] font-bold text-white bg-brand-600 hover:bg-brand-500 active:scale-95 transition-all shadow-lg shadow-brand-500/10"
      onclick={onOpenCreateRoleEditor}
      data-testid="role-create-button"
    >
      + Create
    </button>
  </div>

  {#if !hasCustomRoles}
    <div class="p-4 rounded-xl border-2 border-dashed flex flex-col items-center justify-center text-center space-y-2 {dark ? 'border-zinc-800 bg-white/[0.01]' : 'border-zinc-200 bg-black/[0.01]'}" data-testid="role-custom-empty-state">
      <p class="text-xs {t.textMuted}">No custom roles yet. Create one or capture from a live team.</p>
    </div>
  {/if}

  <div class="space-y-3" data-testid="template-role-list">
    {#each filteredRoleTemplates as role, i}
      <article class="group rounded-xl border p-3 transition-all animate-in fade-in slide-in-from-bottom-1 duration-200 {cardTone}" style:transition-delay={`${i * 30}ms`} data-testid={`role-template-card-${role.roleId}`}>
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <p class="truncate text-[14px] font-bold {t.textPrimary}">{role.name}</p>
            <p class="text-[10px] font-mono {toneMuted}">{role.roleId}</p>
          </div>
          <span class="rounded-full px-2 py-0.5 text-[9px] font-bold uppercase tracking-tight {roleKindBadgeTone(role.kind, dark)}">{role.kind}</span>
        </div>

        <div class="mt-3 flex flex-wrap items-center gap-1.5">
          <span
            class="inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[10px] font-bold {dark ? 'border-zinc-700 text-zinc-300 bg-zinc-900/80' : 'border-zinc-200 text-zinc-700 bg-zinc-50'}"
            data-testid={`role-tool-badge-${role.roleId}`}
          >
            <svg class="h-3 w-3 shrink-0" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
              <path d={getToolIcon(role.cliTool).path}></path>
            </svg>
            <span class="uppercase tracking-tighter opacity-80">{getToolName(role.cliTool)}</span>
          </span>
          <span
            class="inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-bold {dark ? 'border-zinc-700 text-zinc-300 bg-zinc-900/80' : 'border-zinc-200 text-zinc-700 bg-zinc-50'}"
            data-testid={`role-model-badge-${role.roleId}`}
          >
            {role.model || 'unspecified'}
          </span>
        </div>

        <div class="mt-3 space-y-2">
          <div class="rounded-lg border px-2.5 py-2 {dark ? 'border-white/8 bg-white/[0.02]' : 'border-zinc-200 bg-white/70'}">
            <p class="text-[10px] font-bold uppercase tracking-wide {toneMuted}">Focus area</p>
            <p class="mt-1 text-[12px] font-semibold {t.textPrimary}" data-testid={`role-focus-area-${role.roleId}`}>
              {role.focusArea || 'No focus area defined.'}
            </p>
          </div>
          <div class="rounded-lg border px-2.5 py-2 {dark ? 'border-white/8 bg-white/[0.02]' : 'border-zinc-200 bg-white/70'}">
            <p class="text-[10px] font-bold uppercase tracking-wide {toneMuted}">Behavior summary</p>
            <p class="mt-1 text-[11px] leading-relaxed {t.textSecondary}" data-testid={`role-behavior-summary-${role.roleId}`}>
              {role.behaviorSummary || role.contextSummary || 'No behavior summary available.'}
            </p>
          </div>
        </div>

        <div class="mt-4 flex flex-wrap justify-end gap-2 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
          <div class="flex gap-1.5">
            <button
              class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
              onclick={() => onSelectRole(role)}
              data-testid={`role-use-${role.roleId}`}
            >
              Use
            </button>
            <button
              class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
              onclick={() => {
                onInspectRole(role)
              }}
              data-testid={`role-inspect-${role.roleId}`}
            >
              Inspect
            </button>
          </div>

          {#if isCustomRole(role)}
            <div class="flex gap-1.5 ml-auto">
              <button
                class="h-8 w-8 flex items-center justify-center rounded-lg {actionSecondary}"
                onclick={() => {
                  onOpenEditRoleEditor(role)
                }}
                aria-label="Edit role"
                title="Edit role"
                data-testid={`role-edit-${role.roleId}`}
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/><path d="m15 5 4 4"/></svg>
              </button>
              <button
                class="h-8 w-8 flex items-center justify-center rounded-lg border border-danger-500/20 text-danger-500 hover:bg-danger-500/10 active:scale-95 transition-all"
                onclick={() => onRequestRoleDelete(role)}
                aria-label="Delete role"
                title="Delete role"
                data-testid={`role-delete-${role.roleId}`}
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
              </button>
            </div>
          {/if}
        </div>
      </article>
    {/each}
  </div>
{/if}
