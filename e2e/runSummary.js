import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const localPath = path => path.startsWith('file:') ? fileURLToPath(path) : resolve(path)
const specName = path => relative(process.cwd(), localPath(path))

// WDIO has already applied --spec to config.specs before onPrepare. Our
// manifest and recipes use concrete paths; reject unresolved patterns rather
// than claiming coverage for an inferred selection.
export function selectedSpecFiles(config) {
  const specs = config.specs.flat().map(localPath)
  const excluded = (config.exclude ?? []).map(localPath)
  for (const path of [...specs, ...excluded]) {
    if (!existsSync(path)) throw new Error(`Run accounting requires an existing concrete spec path: ${path}`)
  }
  return [...new Set(specs)].filter(path => !excluded.includes(path))
}

// Exact inherited exclusions, documented in docs/operations/testing-guide.md.
// This list never skips a test; it only explains an observed pending result.
export const declaredTestExclusions = [
  {
    "spec": "e2e/specs/overview-interactions.js",
    "test": "Overview Interactions relationships dismissing a relationship removes the row",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No dismissible relationship row"
  },
  {
    "spec": "e2e/specs/git-workflow.js",
    "test": "Git Workflow navigation range filter appears when commits are session-filtered",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No session range filter, more-page sentinel, empty repository, or selectable second project/commit"
  },
  {
    "spec": "e2e/specs/git-workflow.js",
    "test": "Git Workflow navigation scroll sentinel exists when more commits are available",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No session range filter, more-page sentinel, empty repository, or selectable second project/commit"
  },
  {
    "spec": "e2e/specs/git-workflow.js",
    "test": "Git Workflow navigation empty state shows git-empty when repository has no commits",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No session range filter, more-page sentinel, empty repository, or selectable second project/commit"
  },
  {
    "spec": "e2e/specs/git-workflow.js",
    "test": "Git Workflow position memory selected commit is restored after switching projects and back",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No session range filter, more-page sentinel, empty repository, or selectable second project/commit"
  },
  {
    "spec": "e2e/specs/files-workflow.js",
    "test": "Files Workflow file viewing clicking a .js or .rs file shows code-viewer with highlighted spans",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No matching visible code or binary/image fixture node"
  },
  {
    "spec": "e2e/specs/files-workflow.js",
    "test": "Files Workflow file viewing binary or image file shows appropriate viewer or informational message",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No matching visible code or binary/image fixture node"
  },
  {
    "spec": "e2e/specs/tasks-workflow.js",
    "test": "Tasks Workflow kanban board renders kanban columns when tasks exist",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No task rows in the generated repositories"
  },
  {
    "spec": "e2e/specs/tasks-workflow.js",
    "test": "Tasks Workflow kanban board task rows show non-empty subject text",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No task rows in the generated repositories"
  },
  {
    "spec": "e2e/specs/tasks-workflow.js",
    "test": "Tasks Workflow kanban board task rows contain a tool icon SVG",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No task rows in the generated repositories"
  },
  {
    "spec": "e2e/specs/tasks-workflow.js",
    "test": "Tasks Workflow task detail clicking a task row opens the detail panel",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No task rows in the generated repositories"
  },
  {
    "spec": "e2e/specs/tasks-workflow.js",
    "test": "Tasks Workflow task detail detail panel shows description or sections content",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No task rows in the generated repositories"
  },
  {
    "spec": "e2e/specs/tasks-workflow.js",
    "test": "Tasks Workflow task detail detail close button dismisses the panel",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No task rows in the generated repositories"
  },
  {
    "spec": "e2e/specs/cross-tab-navigation.js",
    "test": "Cross-Tab Navigation project switching preserves active tab when returning to a previously visited project",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): Two distinct selectable project labels are unavailable"
  },
  {
    "spec": "e2e/specs/cross-tab-navigation.js",
    "test": "Cross-Tab Navigation project switching new project defaults to Overview tab",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): Two distinct selectable project labels are unavailable"
  },
  {
    "spec": "e2e/specs/settings-persistence.js",
    "test": "Settings Persistence terminal settings terminal emulator change persists after close and reopen",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): No alternative terminal emulator option"
  },
  {
    "spec": "e2e/specs/mesh-recovery.js",
    "test": "Mesh Recovery surfaces degraded runtime state after a member pane dies",
    "reason": "Known issue: team-daemon resume startup verification times out; docs/operations/mesh-flake-audit.md (2026-09-06: 3/3 failures)"
  },
  {
    "spec": "e2e/specs/mesh-recovery.js",
    "test": "Mesh Recovery surfaces duplicate-add conflicts and lets the operator recover by changing the name",
    "reason": "Known issue: team-daemon resume startup verification times out; docs/operations/mesh-flake-audit.md (2026-09-06: 3/3 failures)"
  },
  {
    "spec": "e2e/specs/mesh-recovery.js",
    "test": "Mesh Recovery records when mesh recovery tier 2 is unavailable",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): Inverse condition: isolated worker has mesh and tmux available"
  },
  {
    "spec": "e2e/specs/mesh-workflow.js",
    "test": "Mesh Workflow tier 1 shows blocking availability messaging when mesh is unavailable",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): Inverse condition: isolated worker has mesh and tmux available"
  },
  {
    "spec": "e2e/specs/mesh-workflow.js",
    "test": "Mesh Workflow tier 2 skips tier 2 when mesh prerequisites are unavailable",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): Inverse condition: isolated worker has mesh and tmux available"
  },
  {
    "spec": "e2e/specs/regressions.js",
    "test": "Regressions DirectoryBrowser overflow (commit 284bd54 regression) directory tree has overflow-hidden for horizontal clipping",
    "reason": "Inherited pre-reform conditional skip; phase 3 (liveness/progress pack): DirectoryBrowser is not mounted"
  },
  {
    "spec": "e2e/specs/mesh-recovery.js",
    "test": "Mesh Recovery shows cold-resume controls after a full team stop and reload",
    "reason": "Known issue: team-daemon resume startup verification times out; docs/operations/mesh-flake-audit.md (2026-09-06: 3/3 failures)"
  }
]

export function summarizeSuite(root) {
  const files = {}
  function visit(suite) {
    for (const test of suite.tests) {
      const key = test.file
      const row = files[key] ??= { selected: 0, executed: 0, passed: 0, failed: 0, skipped: 0, unreached: 0, skipped_tests: [], excluded_tests: [] }
      row.selected++
      if (test.state === 'passed' || test.state === 'failed') {
        row.executed++
        row[test.state]++
      } else if (test.pending) {
        row.skipped++
        const title = test.fullTitle?.() ?? test.title
        row.skipped_tests.push(title)
        const exclusion = declaredTestExclusions.find(entry => entry.spec === specName(key) && entry.test === title)
        if (exclusion) row.excluded_tests.push({ test: title, reason: exclusion.reason })
      } else row.unreached++
    }
    suite.suites.forEach(visit)
  }
  visit(root)
  return files
}

export function coverageComplete(specs, exitCode) {
  return exitCode === 0 && Object.keys(specs).length > 0 && Object.entries(specs).every(([file, row]) => {
    if (!row || row.selected === 0 || row.failed !== 0 || row.unreached !== 0) return false
    const explained = (row.skipped_tests ?? []).filter(title => declaredTestExclusions.some(
      entry => entry.spec === specName(file) && entry.test === title
    )).length
    return row.skipped === explained && row.executed + row.skipped === row.selected && row.passed === row.executed
  })
}

export function finishRun(summary, exitCode, finishedAt = Date.now()) {
  summary.exit_code = exitCode
  summary.wall_ms = finishedAt - Date.parse(summary.started_at)
  summary.complete = coverageComplete(summary.specs, exitCode)
}

export function updateRunSummary(update) {
  const path = process.env.E2E_RUN_SUMMARY
  const summary = JSON.parse(readFileSync(path, 'utf8'))
  update(summary)
  writeFileSync(path, `${JSON.stringify(summary, null, 2)}\n`)
  return summary
}

let rootSuite
function snapshot() {
  updateRunSummary(summary => {
    for (const [file, row] of Object.entries(summarizeSuite(rootSuite))) {
      summary.specs[specName(file)] = row
    }
  })
}

// Mocha's complete in-memory tree includes tests not reached after bail and
// dynamic skips. No source parsing, reporter framework, or inferred pass counts.
export const mochaHooks = {
  beforeAll() {
    rootSuite = this.test.parent
    snapshot()
    if (process.env.E2E_SETUP_ERROR) throw new Error(process.env.E2E_SETUP_ERROR)
  },
  afterEach() { snapshot() },
  afterAll() { snapshot() },
}
