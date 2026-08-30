// Static check for the versioned Workflow scripts in .claude/workflows.
// Parses each script (no execution) and enforces the constraints the Workflow
// runtime puts on them: `export const meta` first, no imports, and none of the
// globals that throw inside a workflow script.
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { fileURLToPath } from 'node:url'

export const DEFAULT_WORKFLOWS_DIR = '.claude/workflows'
const SHARED_WORKFLOWS = ['feature-pr.js', 'small-change.js', 'fix-round.js', 'research-sweep.js', 'docs-sweep.js']
const LIB_START = '// ── lib:'
const LIB_END = '// ── end lib ──'

// Workflow scripts run as one inline script with the API injected as globals.
const BANNED = [
  [/^[ \t]*import\s+[^(]/m, 'uses `import` — workflow scripts cannot import; copy the shared lib section instead'],
  [/\brequire\s*\(/, 'uses `require(` — workflow scripts cannot import; copy the shared lib section instead'],
  [/\bDate\.now\s*\(/, 'uses `Date.now()` — it throws in a workflow script (it would break resume); pass a timestamp via args'],
  [/\bnew\s+Date\s*\(\s*\)/, 'uses argless `new Date()` — it throws in a workflow script; pass a timestamp via args'],
  [/\bMath\.random\s*\(/, 'uses `Math.random()` — it throws in a workflow script; vary the prompt or label by index instead'],
]

// Strips whitespace and leading comments so `export const meta` can be checked
// as the first statement without pulling in a parser.
function withoutLeadingTrivia(source) {
  let rest = source
  for (;;) {
    const trimmed = rest.replace(/^\s+/, '')
    if (trimmed.startsWith('//')) {
      rest = trimmed.slice(trimmed.indexOf('\n') + 1)
      continue
    }
    if (trimmed.startsWith('/*')) {
      const end = trimmed.indexOf('*/')
      if (end === -1) return trimmed
      rest = trimmed.slice(end + 2)
      continue
    }
    return trimmed
  }
}

const API_GLOBALS = 'agent, parallel, pipeline, phase, log, workflow, args, budget'
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor

// The script body a parser has to accept: top-level `await`, top-level `return`,
// and the single `export const meta` (stripped, because a function body has no exports).
export function parseBody(source) {
  const head = withoutLeadingTrivia(source)
  const trivia = source.slice(0, source.length - head.length)
  return /^export\s+const\s+meta\b/.test(head) ? trivia + head.replace(/^export\s+/, '') : source
}

export function wrapForParse(source) {
  return `(async function workflowScript(${API_GLOBALS}) {\n${parseBody(source)}\n})`
}

// Best-effort line number: the Function constructor reports none, so re-compile the same
// source through vm.Script, which locates it under node. Under bun that stays silent and the
// problem is reported without a line — better than JSC's number, which points at the caller.
function errorLine(source) {
  try {
    new vm.Script(wrapForParse(source), { filename: 'workflow', lineOffset: -1 })
  } catch (fromVm) {
    const line = /:(\d+)$/.exec(String(fromVm.stack || '').split('\n')[0])
    if (line) return Number(line[1])
  }
  return null
}

function syntaxProblem(name, source) {
  try {
    // Compiles the body only — the Function constructor never runs it. vm.Script is not
    // the detector here: bun compiles its scripts lazily and reports nothing.
    new AsyncFunction(API_GLOBALS, parseBody(source))
    return null
  } catch (error) {
    const line = errorLine(source)
    return `${name}${line ? `:${line}` : ''} syntax error: ${error.message}`
  }
}

// Guards against a runtime that does not report syntax errors at compile time: without
// this the lint would pass everything and read as green.
export function detectorWorks() {
  return syntaxProblem('probe.js', 'export const meta = { name: "probe", description: "probe" }\nconst broken = (\n') !== null
}

export function checkWorkflowSource(fileName, source) {
  const problems = []
  const syntax = syntaxProblem(fileName, source)
  if (syntax) problems.push(syntax)

  const head = withoutLeadingTrivia(source)
  if (!/^export\s+const\s+meta\s*=\s*\{/.test(head)) {
    problems.push(`${fileName}: must start with \`export const meta = {\` (the runtime reads meta before running the script)`)
    return problems
  }

  const expected = path.basename(fileName, '.js')
  const declaredName = /\bname:\s*(['"])([^'"]+)\1/.exec(head)
  if (!declaredName) {
    problems.push(`${fileName}: meta.name is missing`)
  } else if (declaredName[2] !== expected) {
    problems.push(`${fileName}: meta.name '${declaredName[2]}' does not match the file name '${expected}'`)
  }
  if (!/\bdescription:\s*['"]/.test(head)) {
    problems.push(`${fileName}: meta.description is missing (it is shown in the permission dialog)`)
  }

  for (const [pattern, message] of BANNED) {
    if (pattern.test(source)) problems.push(`${fileName}: ${message}`)
  }

  // Required gates must come from the same typed catalog that validates and declares commands.
  // A hand-built array would silently restore the old split sources of truth.
  if (/\bconst\s+REQUIRED_GATES\b/.test(source)) {
    const buildsCatalog = /\bconst\s+GATE_CATALOG\s*=\s*buildGateCatalog\s*\(/.test(source)
    const readsCatalog = /\bconst\s+REQUIRED_GATES\s*=\s*GATE_CATALOG\.required\b/.test(source)
    if (!buildsCatalog || !readsCatalog) {
      problems.push(`${fileName}: REQUIRED_GATES must be built through buildGateCatalog() and read from GATE_CATALOG.required`)
    }
  }

  // Every object in a `*_SCHEMA` literal must close itself with
  // `additionalProperties: false`: the OpenAI structured-output endpoint behind
  // `codex exec --output-schema` rejects a schema that leaves it out, so a Codex
  // review lane would fail before reading a single file.
  for (const block of source.matchAll(/const\s+(\w+_SCHEMA)\s*=\s*\{[\s\S]*?\n\}/g)) {
    const objects = (block[0].match(/type:\s*'object'/g) || []).length
    const closed = (block[0].match(/additionalProperties:\s*false/g) || []).length
    if (objects > closed) {
      problems.push(`${fileName}: ${block[1]} has ${objects} object schema(s) but only ${closed} declare \`additionalProperties: false\` — Codex's --output-schema rejects the rest`)
    }
  }
  return problems
}

export function checkWorkflowDir(dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    return { checked: 0, problems: [`${dir}: workflow directory not found`] }
  }
  const files = fs
    .readdirSync(dir)
    .filter((file) => file.endsWith('.js'))
    .sort()
  if (files.length === 0) {
    return { checked: 0, problems: [`${dir}: no workflow scripts found`] }
  }
  const sources = new Map(files.map((file) => [file, fs.readFileSync(path.join(dir, file), 'utf8')]))
  const problems = files.flatMap((file) => checkWorkflowSource(file, sources.get(file)))
  const shared = SHARED_WORKFLOWS.filter((file) => sources.has(file))
  if (shared.length > 0) {
    const missing = SHARED_WORKFLOWS.filter((file) => !sources.has(file))
    if (missing.length > 0) {
      problems.push(`${dir}: shared workflow set is incomplete; missing ${missing.join(', ')}`)
    } else {
      const block = (file) => {
        const source = sources.get(file)
        const start = source.indexOf(LIB_START)
        const end = source.indexOf(LIB_END)
        if (start === -1 || end <= start) {
          problems.push(`${file}: shared lib block markers are missing or out of order`)
          return null
        }
        return source.slice(start, end)
      }
      const canonical = block('feature-pr.js')
      for (const file of SHARED_WORKFLOWS.slice(1)) {
        const candidate = block(file)
        if (canonical != null && candidate != null && candidate !== canonical) {
          problems.push(`${file}: shared lib block differs byte-for-byte from feature-pr.js`)
        }
      }
    }
  }
  return { checked: files.length, problems }
}

function main() {
  if (!detectorWorks()) {
    console.error('workflow script check cannot run: this JS runtime does not report syntax errors at compile time')
    process.exit(1)
  }
  const dir = process.argv[2] || DEFAULT_WORKFLOWS_DIR
  const { checked, problems } = checkWorkflowDir(dir)
  if (problems.length > 0) {
    for (const problem of problems) console.error(problem)
    console.error(`workflow script check failed: ${problems.length} problem(s) in ${dir}`)
    process.exit(1)
  }
  console.log(`workflow script check passed: ${checked} script(s) in ${dir}`)
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (import.meta.main === true || invokedPath === fileURLToPath(import.meta.url)) {
  main()
}
