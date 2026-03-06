import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import RoleCatalog from './RoleCatalog.svelte'

const t = {
  textPrimary: 'text-zinc-900',
  textSecondary: 'text-zinc-700',
  textMuted: 'text-zinc-500',
}

function sampleRole(overrides = {}) {
  return {
    roleId: 'mesh-expert',
    name: 'Mesh Expert',
    kind: 'agent',
    cliTool: 'gemini',
    model: 'gemini-2.5-pro',
    focusArea: 'Mesh orchestration',
    contextSummary: 'Owns cross-repo protocol awareness and runtime coordination constraints.',
    behaviorSummary: 'Advises on mesh boundaries and avoids unrelated product edits.',
    instructions: 'Long raw instructions that should stay in deeper inspection.',
    capabilities: ['protocols', 'runtime'],
    builtIn: true,
    readOnly: true,
    ...overrides,
  }
}

function renderCatalog(props = {}) {
  return render(RoleCatalog, {
    props: {
      dark: false,
      t,
      cardTone: 'bg-white border-zinc-200',
      actionSecondary: 'bg-zinc-100 border-zinc-200 text-zinc-700',
      toneMuted: 'text-zinc-500',
      detailKind: '',
      detailLoading: false,
      selectedRole: null,
      filteredRoleTemplates: [sampleRole()],
      hasCustomRoles: true,
      onSelectRole: vi.fn(),
      onResetDetail: vi.fn(),
      onOpenCreateRoleEditor: vi.fn(),
      onInspectRole: vi.fn(),
      onOpenEditRoleEditor: vi.fn(),
      onRequestRoleDelete: vi.fn(),
      ...props,
    },
  })
}

describe('RoleCatalog', () => {
  it('renders focus area and behavior summary on role cards without capability chips', () => {
    renderCatalog()

    expect(screen.getByText('Focus area')).toBeInTheDocument()
    expect(screen.getByText('Mesh orchestration')).toBeInTheDocument()
    expect(screen.getByText('Advises on mesh boundaries and avoids unrelated product edits.')).toBeInTheDocument()
    expect(screen.queryByText('protocols')).not.toBeInTheDocument()
    expect(screen.queryByText('runtime')).not.toBeInTheDocument()
  })

  it('renders role detail with context steering metadata and demoted raw instructions', async () => {
    const onResetDetail = vi.fn()
    renderCatalog({
      detailKind: 'role',
      selectedRole: sampleRole(),
      onResetDetail,
    })

    expect(screen.getByTestId('template-role-detail')).toBeInTheDocument()
    expect(screen.getByText('Context summary')).toBeInTheDocument()
    expect(screen.getByText('Behavioral boundary')).toBeInTheDocument()
    expect(screen.getByText('Owns cross-repo protocol awareness and runtime coordination constraints.')).toBeInTheDocument()
    expect(screen.getByText('Advises on mesh boundaries and avoids unrelated product edits.')).toBeInTheDocument()
    expect(screen.getByText('Raw instructions')).toBeInTheDocument()
    expect(screen.queryByText('protocols')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('template-role-back'))
    expect(onResetDetail).toHaveBeenCalledTimes(1)
  })

  it('keeps focus and behavior metadata visible in both light and dark themes', async () => {
    const view = renderCatalog({ dark: false })
    expect(screen.getByTestId('role-focus-area-mesh-expert')).toHaveTextContent('Mesh orchestration')

    await view.rerender({
      dark: true,
      t,
      cardTone: 'bg-zinc-950 border-white/10',
      actionSecondary: 'bg-white/5 border-white/10 text-zinc-300',
      toneMuted: 'text-zinc-400',
      detailKind: '',
      detailLoading: false,
      selectedRole: null,
      filteredRoleTemplates: [sampleRole()],
      hasCustomRoles: true,
      onSelectRole: vi.fn(),
      onResetDetail: vi.fn(),
      onOpenCreateRoleEditor: vi.fn(),
      onInspectRole: vi.fn(),
      onOpenEditRoleEditor: vi.fn(),
      onRequestRoleDelete: vi.fn(),
    })

    expect(screen.getByTestId('role-focus-area-mesh-expert')).toHaveTextContent('Mesh orchestration')
    expect(screen.getByTestId('role-behavior-summary-mesh-expert')).toHaveTextContent(
      'Advises on mesh boundaries and avoids unrelated product edits.'
    )
  })
})
