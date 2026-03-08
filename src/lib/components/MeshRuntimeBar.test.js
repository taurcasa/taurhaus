import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshRuntimeBar from './MeshRuntimeBar.svelte'

const agents = [
  { id: 'a1', status: 'active' },
  { id: 'a2', status: 'active' },
  { id: 'a3', status: 'idle' },
  { id: 'a4', status: 'offline' },
]

describe('MeshRuntimeBar', () => {
  it('shows team name and status summary counts', () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'degraded',
      },
    })

    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent(
      '5 members • 3 active • 1 idle • 1 stopped'
    )
  })

  it('calls onAddAgent when add button is clicked for active teams', async () => {
    const onAddAgent = vi.fn()

    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'active',
        onAddAgent,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    expect(onAddAgent).toHaveBeenCalledTimes(1)
  })

  it('shows disband inside the overflow menu, not as a primary action', async () => {
    const onDisband = vi.fn()

    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'offline' },
        agents,
        teamRuntimeState: 'coldResume',
        onDisband,
      },
    })

    expect(screen.queryByTestId('mesh-runtime-disband')).not.toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('mesh-runtime-more-toggle'))
    expect(screen.getByTestId('mesh-runtime-stop-all')).toBeDisabled()
    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    expect(onDisband).toHaveBeenCalledTimes(1)
  })

  it('renders recent compaction reinjection audit entries when present', () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'active',
        compactionAudit: [
          {
            memberName: 'frontend-dev',
            tool: 'codex',
            lastSessionId: 'session-1',
            lastCompactionTimestamp: '2026-03-08T14:46:41.037Z',
            lastDeliveryResult: 'injected',
          },
        ],
      },
    })

    expect(screen.getByTestId('mesh-runtime-compaction-audit')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-runtime-compaction-entry-frontend-dev')).toHaveTextContent(
      'frontend-dev'
    )
    expect(screen.getByTestId('mesh-runtime-compaction-entry-frontend-dev')).toHaveTextContent(
      'Codex'
    )
    expect(screen.getByText('Injected')).toBeInTheDocument()
    expect(screen.getByText('2026-03-08T14:46:41.037Z')).toBeInTheDocument()
  })

  it('renders compaction diagnostics when present', () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'active',
        compactionDiagnostics: {
          extractor: {
            heartbeatAt: '2026-03-08T15:04:18Z',
            lastProcessedSignalId: 'sig-123',
            activeFiles: [
              {
                jsonlPath: '/home/mstie/.codex/sessions/2026/03/08/run.jsonl',
                offset: 321,
                lastError: 'tail read failed once',
              },
            ],
          },
          signalLog: {
            signalLogPath: '/tmp/teams/architecture-final/state/compaction/signals/codex-compaction-signals.jsonl',
            totalSignals: 4,
            unconsumedCount: 1,
            lastConsumedOffset: 256,
            recentSignals: [
              {
                signalId: 'sig-123',
                emittedAt: '2026-03-08T15:04:18Z',
                sessionId: 'session-1',
                paneId: '%12',
                projectPath: '/home/mstie/projects/taurhaus',
                transcriptTimestamp: '2026-03-08T15:04:17Z',
                signalKind: 'context_compacted',
              },
            ],
          },
          watcher: {
            lastEventAt: '2026-03-08T15:04:19Z',
            reconciliationPollCount: 7,
            missedEventRecoveryCount: 2,
          },
        },
      },
    })

    expect(screen.getByTestId('mesh-runtime-compaction-diagnostics')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-runtime-compaction-extractor')).toHaveTextContent(
      '1 active file'
    )
    expect(screen.getByTestId('mesh-runtime-compaction-extractor-files')).toHaveTextContent(
      'tail read failed once'
    )
    expect(screen.getByTestId('mesh-runtime-compaction-signal-log')).toHaveTextContent(
      '4 total, 1 unconsumed'
    )
    expect(screen.getByTestId('mesh-runtime-compaction-watcher')).toHaveTextContent(
      'reconciles 7'
    )
    expect(screen.getByTestId('mesh-runtime-compaction-signals')).toHaveTextContent(
      'Context compacted'
    )
  })
})
