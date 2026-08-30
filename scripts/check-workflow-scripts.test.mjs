// @vitest-environment node
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { checkWorkflowSource, checkWorkflowDir, detectorWorks } from './check-workflow-scripts.mjs'

const WORKFLOWS = fileURLToPath(new URL('../.claude/workflows', import.meta.url))

const VALID = `export const meta = {
  name: 'demo',
  description: 'Demo workflow',
  phases: [{ title: 'Work' }],
}

const A = args || {}
phase('Work')
const out = await agent('do the thing', { label: 'work', phase: 'Work', model: 'opus' })
log('done')
return { out }
`

function titles(problems) {
  return problems.join('\n')
}

describe('workflow script check', () => {
  it('accepts a script with meta first, top-level await and top-level return', () => {
    expect(checkWorkflowSource('demo.js', VALID)).toEqual([])
  })

  it('confirms the runtime reports syntax errors at compile time', () => {
    // Regression: bun's vm.Script compiles lazily and reported nothing, so the lint passed a
    // script with an unclosed paren. The detector now compiles through the Function
    // constructor, and this guard fails loudly on a runtime that still stays silent.
    expect(detectorWorks()).toBe(true)
  })

  it('reports a syntax error with the offending line', () => {
    const broken = VALID.replace("log('done')", "log('done'")
    const problems = checkWorkflowSource('demo.js', broken)
    expect(problems.length).toBe(1)
    expect(titles(problems)).toMatch(/syntax/i)
    expect(titles(problems)).toMatch(/demo\.js:1[0-9]/)
  })

  it('requires export const meta as the first statement', () => {
    const late = `const SP = '/tmp/x'\n${VALID}`
    expect(titles(checkWorkflowSource('demo.js', late))).toMatch(/export const meta/)
  })

  it('allows leading comments before export const meta', () => {
    expect(checkWorkflowSource('demo.js', `// a note\n/* block */\n${VALID}`)).toEqual([])
  })

  it('requires meta.name to match the file name', () => {
    const renamed = VALID.replace("name: 'demo'", "name: 'other'")
    expect(titles(checkWorkflowSource('demo.js', renamed))).toMatch(/meta\.name 'other'.*'demo'/)
  })

  it('requires a description', () => {
    const bare = VALID.replace("  description: 'Demo workflow',\n", '')
    expect(titles(checkWorkflowSource('demo.js', bare))).toMatch(/description/)
  })

  it('rejects imports and require calls (workflow scripts cannot import)', () => {
    expect(titles(checkWorkflowSource('demo.js', VALID.replace('const A =', "import fs from 'node:fs'\nconst A =")))).toMatch(
      /import/
    )
    expect(titles(checkWorkflowSource('demo.js', VALID.replace('const A =', "const fs = require('node:fs')\nconst A =")))).toMatch(
      /require/
    )
  })

  it('rejects Date.now(), argless new Date() and Math.random()', () => {
    expect(titles(checkWorkflowSource('demo.js', VALID.replace('const A =', 'const t = Date.now()\nconst A =')))).toMatch(
      /Date\.now/
    )
    expect(titles(checkWorkflowSource('demo.js', VALID.replace('const A =', 'const t = new Date()\nconst A =')))).toMatch(
      /new Date/
    )
    expect(titles(checkWorkflowSource('demo.js', VALID.replace('const A =', 'const r = Math.random()\nconst A =')))).toMatch(
      /Math\.random/
    )
    expect(checkWorkflowSource('demo.js', VALID.replace('const A =', 'const t = new Date(args.now)\nconst A ='))).toEqual([])
  })

  it('checks every .js file in a directory and reports the count', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-check-'))
    fs.writeFileSync(path.join(dir, 'demo.js'), VALID)
    fs.writeFileSync(path.join(dir, 'other.js'), VALID.replace("name: 'demo'", "name: 'other'"))
    fs.writeFileSync(path.join(dir, 'README.md'), '# not a script')
    const result = checkWorkflowDir(dir)
    expect(result.checked).toBe(2)
    expect(result.problems).toEqual([])
    fs.rmSync(dir, { recursive: true, force: true })
  })

  it('fails when the directory is missing or holds no scripts', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-check-'))
    expect(titles(checkWorkflowDir(path.join(dir, 'nope')).problems)).toMatch(/not found/)
    expect(titles(checkWorkflowDir(dir).problems)).toMatch(/no workflow scripts/)
    fs.rmSync(dir, { recursive: true, force: true })
  })

  it('requires REQUIRED_GATES to come from the typed gate catalog helper', () => {
    const direct = VALID.replace(
      "const A = args || {}",
      "const A = args || {}\nconst REQUIRED_GATES = ['just check-quick', 'just lint']"
    )
    expect(titles(checkWorkflowSource('demo.js', direct))).toMatch(/REQUIRED_GATES.*buildGateCatalog/i)

    const typed = VALID.replace(
      "const A = args || {}",
      "const A = args || {}\nfunction buildGateCatalog() { return { required: [] } }\nconst GATE_CATALOG = buildGateCatalog(A.gates, A.requiredGates)\nconst REQUIRED_GATES = GATE_CATALOG.required"
    )
    expect(checkWorkflowSource('demo.js', typed)).toEqual([])
  })

  it('fails when one of the five procedure lib blocks drifts', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-check-'))
    const workflows = ['feature-pr.js', 'small-change.js', 'fix-round.js', 'research-sweep.js', 'docs-sweep.js']
    for (const workflow of workflows) {
      fs.copyFileSync(path.join(WORKFLOWS, workflow), path.join(dir, workflow))
    }
    const drifted = path.join(dir, 'small-change.js')
    fs.writeFileSync(drifted, fs.readFileSync(drifted, 'utf8').replace('const DEFAULT_GATES =', 'const DEFAULT_GATES  ='))
    expect(titles(checkWorkflowDir(dir).problems)).toMatch(/small-change\.js.*shared lib.*feature-pr\.js/i)
    fs.rmSync(dir, { recursive: true, force: true })
  })
})

// Regression: the five procedures shipped schema literals without
// `additionalProperties: false`; the first Codex review lane run through
// feature-pr failed closed at the API boundary (invalid_json_schema) before
// reading a file.
describe('strict schema literals', () => {
  const base = "export const meta = { name: 'demo', description: 'x' }\n"
  it('reports a schema object without additionalProperties: false', () => {
    const open = base + "const FINDINGS_SCHEMA = {\n  type: 'object',\n  required: ['a'],\n  properties: { a: { type: 'object', properties: {} } },\n}\nreturn FINDINGS_SCHEMA\n"
    const problems = checkWorkflowSource('demo.js', open)
    expect(problems.some((p) => p.includes('FINDINGS_SCHEMA') && p.includes('additionalProperties'))).toBe(true)
  })
  it('accepts a schema whose objects all close themselves', () => {
    const closed = base + "const FINDINGS_SCHEMA = {\n  type: 'object',\n  additionalProperties: false,\n  required: ['a'],\n  properties: { a: { type: 'object', additionalProperties: false, properties: {} } },\n}\nreturn FINDINGS_SCHEMA\n"
    expect(checkWorkflowSource('demo.js', closed).filter((p) => p.includes('additionalProperties'))).toEqual([])
  })
})
