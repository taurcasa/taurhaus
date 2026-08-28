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
    cliTool: 'agy',
    model: 'gemini-3.7-flash-high',
    focusArea: 'Mesh orchestration',
    contextSummary: 'Owns cross-repo protocol awareness and runtime coordination constraints.',
    behaviorSummary: 'Advises on mesh boundaries and avoids unrelated product edits.',
    instructions: 'Long raw instructions that should stay in deeper inspection.',
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
      onImportRole: vi.fn(),
      onInspectRole: vi.fn(),
      onExportRole: vi.fn(),
      onOpenEditRoleEditor: vi.fn(),
      onRequestRoleDelete: vi.fn(),
      exportingRoleId: '',
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

  it('renders the import button and forwards clicks', async () => {
    const onImportRole = vi.fn()
    renderCatalog({ onImportRole })

    expect(screen.getByTestId('role-import-button')).toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('role-import-button'))

    expect(onImportRole).toHaveBeenCalledTimes(1)
  })

  it('renders a provenance badge only for imported roles', () => {
    renderCatalog({
      filteredRoleTemplates: [
        sampleRole({
          roleId: 'imported-role',
          provenance: {
            sourceFormat: 'claude_agent',
            sourcePath: '.claude/agents/imported-role.md',
            importedAt: '2026-03-08T10:11:12Z',
            nonRoundtrippableFields: ['constraints'],
          },
        }),
        sampleRole({
          roleId: 'native-role',
          name: 'Native Role',
          provenance: null,
        }),
      ],
    })

    expect(screen.getByTestId('role-provenance-badge-imported-role')).toHaveTextContent(
      'from Claude Code'
    )
    expect(screen.queryByTestId('role-provenance-badge-native-role')).not.toBeInTheDocument()
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

  it('renders provenance metadata in the role detail panel when available', () => {
    renderCatalog({
      detailKind: 'role',
      selectedRole: sampleRole({
        provenance: {
          sourceFormat: 'copilot_agent',
          sourcePath: '.github/agents/mesh-expert.md',
          importedAt: '2026-03-07T21:04:05Z',
          nonRoundtrippableFields: ['constraints', 'behavioral_contract'],
        },
      }),
    })

    expect(screen.getByTestId('role-provenance-section')).toBeInTheDocument()
    expect(screen.getByText('Imported format')).toBeInTheDocument()
    expect(screen.getByText('Copilot')).toBeInTheDocument()
    expect(screen.getByText('.github/agents/mesh-expert.md')).toBeInTheDocument()
    expect(screen.getByText('2026-03-07')).toBeInTheDocument()
    expect(screen.getByText('constraints, behavioral_contract')).toBeInTheDocument()
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
      onImportRole: vi.fn(),
      onInspectRole: vi.fn(),
      onExportRole: vi.fn(),
      onOpenEditRoleEditor: vi.fn(),
      onRequestRoleDelete: vi.fn(),
      exportingRoleId: '',
    })

    expect(screen.getByTestId('role-focus-area-mesh-expert')).toHaveTextContent('Mesh orchestration')
    expect(screen.getByTestId('role-behavior-summary-mesh-expert')).toHaveTextContent(
      'Advises on mesh boundaries and avoids unrelated product edits.'
    )
  })

  it('renders export actions on role cards and in role detail', async () => {
    const onExportRole = vi.fn()

    const cardView = renderCatalog({ onExportRole })

    await fireEvent.click(screen.getByTestId('role-export-trigger-mesh-expert'))
    expect(screen.getByTestId('role-export-menu-mesh-expert')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('role-export-format-mesh-expert-claude_agent'))
    expect(onExportRole).toHaveBeenCalledWith(expect.objectContaining({ roleId: 'mesh-expert' }), 'claude_agent')

    onExportRole.mockClear()
    cardView.unmount()

    renderCatalog({
      detailKind: 'role',
      selectedRole: sampleRole(),
      onExportRole,
    })

    await fireEvent.click(screen.getByTestId('role-detail-export-mesh-expert'))
    expect(screen.getByTestId('role-detail-export-menu-mesh-expert')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('role-detail-export-format-mesh-expert-copilot_agent'))
    expect(onExportRole).toHaveBeenCalledWith(expect.objectContaining({ roleId: 'mesh-expert' }), 'copilot_agent')
  })
})
