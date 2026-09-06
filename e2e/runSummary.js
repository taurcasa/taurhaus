import { ConfigParser } from '@wdio/config/node'
import { readFileSync, writeFileSync } from 'node:fs'
import { relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const localPath = path => path.startsWith('file:') ? fileURLToPath(path) : resolve(path)
const specName = path => relative(process.cwd(), localPath(path))

// Use WDIO's own selector so --spec/--exclude/grouping cannot drift from the runner.
export function selectedSpecFiles(config) {
  const parser = new ConfigParser(resolve('e2e/wdio.conf.js'))
  parser.merge(config)
  return parser.getSpecs().flat().map(localPath)
}

export function summarizeSuite(root) {
  const files = {}
  function visit(suite) {
    for (const test of suite.tests) {
      const key = test.file
      const row = files[key] ??= { selected: 0, executed: 0, passed: 0, failed: 0, skipped: 0, unreached: 0, skipped_tests: [] }
      row.selected++
      if (test.state === 'passed' || test.state === 'failed') {
        row.executed++
        row[test.state]++
      } else if (test.pending) {
        row.skipped++
        row.skipped_tests.push(test.fullTitle?.() ?? test.title)
      } else row.unreached++
    }
    suite.suites.forEach(visit)
  }
  visit(root)
  return files
}

export function coverageComplete(specs, exitCode) {
  return exitCode === 0 && Object.keys(specs).length > 0 && Object.values(specs).every(row =>
    row && row.selected > 0 && row.executed === row.selected && row.passed === row.selected &&
    row.failed === 0 && row.skipped === 0 && row.unreached === 0
  )
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
