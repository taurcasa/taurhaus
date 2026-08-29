import { invokeOrMock } from './client.js'
import {
  MOCK_WORKFLOW_LEDGER_ROW,
  MOCK_WORKFLOW_RUN,
  MOCK_WORKFLOW_RUNS,
} from './mocks/workflows.js'

export function listWorkflowRuns(sessionId) {
  return invokeOrMock('list_workflow_runs', { sessionId }, () => MOCK_WORKFLOW_RUNS)
}

export function getWorkflowRun(sessionId, runId) {
  return invokeOrMock('get_workflow_run', { sessionId, runId }, () => MOCK_WORKFLOW_RUN)
}

export function workflowLedgerRow(sessionId, runId) {
  return invokeOrMock('workflow_ledger_row', { sessionId, runId }, () => MOCK_WORKFLOW_LEDGER_ROW)
}
