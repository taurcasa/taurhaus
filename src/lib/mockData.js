/**
 * Backward-compatible re-export for existing imports.
 * Mock fixtures now live under src/lib/ipc/mocks/.
 */

export {
  MOCK_PROJECTS,
  MOCK_COMMITS,
  MOCK_DIFF_HUNKS,
  MOCK_FILE_TREE,
  MOCK_SESSION,
  MOCK_SESSIONS,
  MOCK_DETAIL,
  MOCK_SEARCH_RESULTS,
  MOCK_RELATIONSHIPS,
  MOCK_SETTINGS,
  MOCK_CLAUDE_SESSIONS,
} from './ipc/mocks/base.js'
