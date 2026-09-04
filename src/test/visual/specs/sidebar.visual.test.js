import { screen } from '@testing-library/svelte'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import { commands } from 'vitest/browser'

let currentScenario = null

vi.mock('../../../lib/sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn((projectPath) => currentScenario?.sessionStore?.sessionByProject?.[projectPath] ?? null),
  getSessionsForProject: vi.fn((projectPath) => currentScenario?.sessionStore?.sessionsByProject?.[projectPath] ?? []),
}))

// The ambient account signal is store-derived; scenarios that exercise the
// footer badge force a reading instead of priming the whole account store.
vi.mock('../../../lib/accountPresentation.js', async (importOriginal) => {
  const actual = await importOriginal()
  return {
    ...actual,
    ambientAccountSignal: (...args) =>
      currentScenario?.accountSignal ?? actual.ambientAccountSignal(...args),
  }
})

import Sidebar from '../../../lib/Sidebar.svelte'
import { captureVisualBase64, renderVisual } from '../renderVisual.js'
import { sidebarScenarios } from '../fixtures/sidebar.fixtures.js'

const screenshotsByScenario = new Map()
const viewport = { width: 960, height: 640 }
const viewportClip = { x: 0, y: 0, width: viewport.width, height: viewport.height }

beforeAll(async () => {
  currentScenario = null
})

describe('Sidebar visual coverage', () => {
  it.each(sidebarScenarios)('captures $name', async (scenario) => {
    currentScenario = scenario

    await renderVisual(Sidebar, {
      theme: scenario.theme,
      viewport,
      props: {
        projects: scenario.projects,
        selectedProject: scenario.selectedProject,
        daemonStatus: scenario.daemonStatus,
        ...(scenario.props ?? {}),
      },
    })

    document.documentElement.style.height = `${viewport.height}px`
    document.body.style.height = `${viewport.height}px`
    document.body.style.overflow = 'hidden'

    expect(document.documentElement.dataset.theme).toBe(scenario.theme)
    expect(document.body.dataset.theme).toBe(scenario.theme)
    expect(screen.getByTestId('sidebar-project-scroll')).toBeInTheDocument()

    for (const label of scenario.expected.labels) {
      if (label.includes(':')) {
        expect(screen.getByLabelText(label)).toBeInTheDocument()
      } else {
        expect(screen.getByText(label)).toBeInTheDocument()
      }
    }

    if (scenario.name.startsWith('workflow_runs_')) {
      const badge = screen.getByLabelText('2 workflow runs live')
      expect(badge).toHaveTextContent('2')
      expect(badge.title).toContain('last agent write')
    }

    const selectedRow = screen.getByText(scenario.expected.selectedProjectName).closest('button')
    expect(selectedRow).toBeTruthy()
    if (scenario.expected.rowState === 'held') {
      // A utility surface occupies the panel: the row demotes to held.
      expect(selectedRow.className).not.toContain('sidebar-row-pulled')
      expect(selectedRow.className).toContain('bg-white/[0.06]')
    } else {
      // With no utility surface open the selected row wears the pulled
      // panel material.
      expect(selectedRow.className).toContain('sidebar-row-pulled')
    }

    if (scenario.expected.accountsBadge) {
      expect(screen.getByTestId('accounts-signal')).toHaveTextContent(scenario.expected.accountsBadge)
    }

    const screenshotPath = `sidebar/${scenario.name}.png`
    const screenshotResult = await captureVisualBase64(screenshotPath, { clip: viewportClip })
    const artifact = await commands.readVisualArtifact(screenshotPath)
    const screenshotBase64 = screenshotResult.base64

    screenshotsByScenario.set(scenario.name, screenshotBase64)

    expect(artifact.path.endsWith(screenshotPath)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
    expect(artifact.width).toBe(viewport.width)
    expect(artifact.height).toBe(viewport.height)

    if (scenario.compareAgainst) {
      const baseline = screenshotsByScenario.get(scenario.compareAgainst)
      expect(baseline).toBeTruthy()
      expect(screenshotBase64).not.toBe(baseline)
    }
  })
})
