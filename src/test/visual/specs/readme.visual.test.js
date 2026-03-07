import { fireEvent, screen, waitFor, within } from '@testing-library/svelte'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import { commands } from 'vitest/browser'
import { visualIpcMocks } from '../ipcVisualMocks.js'

let currentSessionState = null

vi.mock('../../../lib/sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn((projectPath) => currentSessionState?.sessionByProject?.[projectPath] ?? null),
  getSessionsForProject: vi.fn((projectPath) => currentSessionState?.sessionsByProject?.[projectPath] ?? []),
}))
import { captureVisual, renderVisual } from '../renderVisual.js'
import ReadmeScreenshotsHost from './ReadmeScreenshotsHost.svelte'
import {
  readmeScreenshotFixtureData,
  readmeScreenshotScenarios,
} from '../fixtures/readmeScreenshots.fixtures.js'

function buildIpcOverrides() {
  return {
    search: async () => readmeScreenshotFixtureData.searchResults,
    getProjectTasks: async () => ({
      tasks: readmeScreenshotFixtureData.tasks,
      errors: [],
    }),
    getTaskDetail: async () => readmeScreenshotFixtureData.taskDetail,
    getAllCommits: async () => readmeScreenshotFixtureData.git.commits,
    getCommitFiles: async () => readmeScreenshotFixtureData.git.commitFiles,
    getCommitDiff: async () => readmeScreenshotFixtureData.git.diffHunks,
    getCommitsInRange: async () => ({
      commits: readmeScreenshotFixtureData.git.commits,
      files: [],
      truncated: false,
      total_count: readmeScreenshotFixtureData.git.commits.length,
    }),
  }
}

beforeAll(() => {
  currentSessionState = null
})

async function settle(ms = 0) {
  await new Promise((resolve) => setTimeout(resolve, ms))
}

describe('README screenshot capture', () => {
  it.each(readmeScreenshotScenarios)('captures $name', async (scenario) => {
    currentSessionState = {
      sessionsByProject: readmeScreenshotFixtureData.sidebarSessionsByProject,
      sessionByProject: Object.fromEntries(
        Object.entries(readmeScreenshotFixtureData.sidebarSessionsByProject)
          .map(([projectPath, sessions]) => [projectPath, sessions[0] ?? null])
      ),
    }

    await renderVisual(ReadmeScreenshotsHost, {
      theme: scenario.theme,
      viewport: scenario.viewport,
      props: {
        scenario,
        fixtureData: readmeScreenshotFixtureData,
      },
      ipc: buildIpcOverrides(),
    })

    if (scenario.hoverProjectName) {
      const row = screen.getAllByTestId('project-item')
        .find((projectItem) => within(projectItem).queryByText(scenario.hoverProjectName))
      expect(row).toBeTruthy()
      await fireEvent.mouseEnter(row)
      await waitFor(() => {
        expect(screen.getByTestId('hovercard')).toBeInTheDocument()
      })
      await settle(120)
    }

    if (scenario.mode === 'tasks') {
      await waitFor(() => {
        expect(screen.getAllByTestId('task-row').length).toBeGreaterThan(0)
      })
      await fireEvent.click(screen.getAllByTestId('task-row')[0])
      await waitFor(() => {
        expect(screen.getByTestId('task-detail-panel')).toBeInTheDocument()
      })
      await settle(120)
    }

    if (scenario.mode === 'search') {
      await waitFor(() => {
        expect(screen.getByTestId('search-input')).toBeInTheDocument()
      })
      const input = screen.getByTestId('search-input')
      await fireEvent.input(input, {
        target: { value: scenario.searchQuery },
      })
      await waitFor(() => {
        expect(visualIpcMocks.search).toHaveBeenCalledWith(scenario.searchQuery, 20)
      }, { timeout: 2_000 })
      await waitFor(() => {
        expect(screen.getByText('README hero copy')).toBeInTheDocument()
      }, { timeout: 2_000 })
      await settle(120)
    }

    if (scenario.mode === 'git') {
      await waitFor(() => {
        expect(screen.getAllByTestId('commit-row').length).toBeGreaterThan(0)
      })
      await fireEvent.click(screen.getAllByTestId('commit-row')[0])
      await waitFor(() => {
        expect(screen.getAllByTestId('commit-file').length).toBeGreaterThan(0)
      })
      await fireEvent.click(screen.getAllByTestId('commit-file')[0])
      await waitFor(() => {
        expect(screen.getByTestId('diff-content')).toBeInTheDocument()
      })
      await settle(120)
    }

    const screenshotPath = `readme/${scenario.fileName}`
    await captureVisual(screenshotPath)
    const artifact = await commands.readVisualArtifact(screenshotPath)

    expect(artifact.path.endsWith(screenshotPath)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
